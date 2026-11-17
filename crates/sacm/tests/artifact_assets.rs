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
//! Slice 0055 (ADR 0034 amendment 1) adds the four remaining assets, read
//! typed in `artifactElement` with every Base feature and their properties:
//! `Artifact` (SACM23-ART-004) with `version` and `date`, `Resource`
//! (SACM23-ART-005) with its `MultiLangString` `location`, `Activity`
//! (SACM23-ART-006) with `startTime` and `endTime`, and `Event`
//! (SACM23-ART-007) with `date`, whose model spelling `occurece` is the
//! `ModelSpelling` tolerance of SACM23-XMI-009. The date-typed values are
//! tested in `artifact_dates.rs`, the creates and sets in `artifact_edits.rs`.
//!
//! Expected locations are marked in the input with [`MARK`] (U+2038), which
//! the test removes before reading. Inputs end lines with LF only.

use sacm::compare::Difference;
use sacm::compare::semantic_compare;
use sacm::diagnostic::{Code, Diagnostic, Severity, SourceLocation};
use sacm::edit::CreateTechnique;
use sacm::edit::{
    self, DeleteElement, EditError, Mutation, Operation, PackageDeletePolicy, ReorderElements,
    SetFlag, SetGid, SetName, SetReferences,
};
use sacm::model::MultiLangString;
use sacm::model::{Document, ElementId, ElementKind, Flag, LangString, ReferenceEnd, SacmElement};
use sacm::validation;
use sacm::xmi::{self, Mode, ReadError, ReadOptions, ReadOutput, WriteError, WriteOptions};

const ART_003: &str = "SACM23-ART-003";
const ART_004: &str = "SACM23-ART-004";
const ART_005: &str = "SACM23-ART-005";
const ART_006: &str = "SACM23-ART-006";
const ART_007: &str = "SACM23-ART-007";
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
    full_with(class, "", "")
}

/// [`full`], with the extra start-tag `attributes` on `x1` after its
/// `abstractForm`, and `content` after its property.
fn full_with(class: &str, attributes: &str, content: &str) -> String {
    in_a(&format!(
        "<artifactElement xmi:type=\"sacm:{class}\" xmi:id=\"x1\" gid=\"G-1\" isCitation=\"true\" \
         citedElement=\"x3\" abstractForm=\"x2\"{attributes}>\n    <name lang=\"en\" content=\"X1\"/>\n    \
         <description xmi:id=\"x1_d\"><content><value lang=\"en\" content=\"Top-down\"/></content>\
         </description>\n    \
         <note xmi:id=\"x1_n\"><content><value content=\"Reviewed\"/></content></note>\n    \
         <taggedValue xmi:id=\"x1_t\"><key><value content=\"Tool\"/></key>\
         <content><value content=\"FTA+\"/></content></taggedValue>\n    \
         <property xmi:id=\"x1_p\"><name content=\"Qualified\"/></property>{content}\n  \
         </artifactElement>\n  \
         <artifactElement xmi:type=\"sacm:{class}\" xmi:id=\"x2\" isAbstract=\"true\">\
         <name content=\"X2\"/></artifactElement>\n  \
         <artifactElement xmi:type=\"sacm:{class}\" xmi:id=\"x3\"><name content=\"X3\"/>\
         </artifactElement>"
    ))
}

fn assert_every_base_feature_round_trips(row: &str, class: &str, kind: ElementKind) {
    assert_every_feature_round_trips(row, class, kind, "", "");
}

/// [`assert_every_base_feature_round_trips`] on [`full_with`], with
/// `attributes` given in the order they are written; returns the document.
fn assert_every_feature_round_trips(
    row: &str,
    class: &str,
    kind: ElementKind,
    attributes: &str,
    content: &str,
) -> Document {
    let what = format!("a {class} with every Base feature");
    let doc = read_ok(row, &what, &full_with(class, attributes, content));
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
                 isCitation=\"true\" citedElement=\"x3\" abstractForm=\"x2\"{attributes}>"
            ),
            "{row}: {what}: the start tag of x1 in {mode:?} mode\n{text}"
        );
    }
    // The same in an AssuranceCasePackage's artifactPackage.
    let nested = full_with(class, attributes, content)
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
    let nested_what = format!("{what}, in an AssuranceCasePackage");
    let nested_doc = read_ok(row, &nested_what, &nested);
    assert_eq!(
        placed(row, &nested_doc, "x1"),
        (kind, Some("a".to_owned())),
        "{row}: {nested_what}: x1 is not in a"
    );
    round_trip(row, &nested_what, &nested_doc, Mode::Strict);
    doc
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

// =====================================================================================
// Slice 0055: Artifact, Resource, Activity and Event (ADR 0034 amendment 1)
// =====================================================================================

/// The worked fixture of slice 0055.
const ASSETS_FIXTURE: &str = concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/tests/data/sacm23/xmi/artifact-assets-valid.sacm.xmi"
);

/// The golden file the fixture writes.
const ASSETS_GOLDEN: &str = concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/tests/data/sacm23/xmi/golden/artifact-assets.golden.sacm.xmi"
);

/// A `location` with an id, holding one value with an id.
const LOCATION: &str = "<location xmi:id=\"l\"><value xmi:id=\"l_en\" lang=\"en\" \
                        content=\"https://example.org/r.pdf\"/></location>";

/// `template` without its marks, and the location of each mark in order:
/// lines end at LF, and columns count characters.
fn marked(row: &str, template: &str) -> (String, Vec<SourceLocation>) {
    let (mut line, mut column) = (1_u32, 1_u32);
    let mut text = String::with_capacity(template.len());
    let mut marks = Vec::new();
    for c in template.chars() {
        if c == MARK {
            marks.push(loc(row, line, column));
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

/// The element `h` of class `class`, typed in `artifactElement` of [`in_a`],
/// with the extra start-tag `attributes`, named "H", holding `content`.
fn host_with(class: &str, attributes: &str, content: &str) -> String {
    in_a(&format!(
        "<artifactElement xmi:type=\"sacm:{class}\" xmi:id=\"h\"{attributes}><name content=\"H\"/>\
         {content}</artifactElement>"
    ))
}

/// The trimmed lines of `text` from the one holding `xmi:id="{id}"`, `count`
/// of them.
fn lines_from(row: &str, text: &str, id: &str, count: usize) -> Vec<String> {
    let attribute = format!("xmi:id=\"{id}\"");
    let lines: Vec<&str> = text.lines().map(str::trim).collect();
    let Some(at) = lines.iter().position(|line| line.contains(&attribute)) else {
        panic!("{row}: the output has no line with {attribute}:\n{text}");
    };
    lines[at..lines.len().min(at + count)]
        .iter()
        .map(|&line| line.to_owned())
        .collect()
}

fn assets_fixture(row: &str) -> String {
    std::fs::read_to_string(ASSETS_FIXTURE)
        .unwrap_or_else(|error| panic!("{row}: cannot read {ASSETS_FIXTURE}: {error}"))
}

fn en(content: &str) -> LangString {
    LangString::new(content).with_lang("en")
}

// --- Item 1 of each row: every feature, read and written ------------------------------

#[test]
fn sacm23_art_004_an_artifact_with_every_feature_reads_in_both_modes_and_writes_back() {
    let doc = assert_every_feature_round_trips(
        ART_004,
        "Artifact",
        ElementKind::Artifact,
        " version=\"1.2\" date=\"2024-03-01\"",
        "",
    );
    {
        let x1 = element(ART_004, &doc, "x1");
        let x2 = element(ART_004, &doc, "x2");
        assert_eq!(
            (x1.version(), x1.date(), x2.version(), x2.date()),
            (Some("1.2"), Some("2024-03-01"), None, None),
            "{ART_004}: the version and date of x1 and x2"
        );
    }
    let _ = doc;
}

#[test]
fn sacm23_art_005_a_resource_with_every_feature_reads_in_both_modes_and_writes_back() {
    let doc =
        assert_every_feature_round_trips(ART_005, "Resource", ElementKind::Resource, "", LOCATION);
    // Given last in the input, the location is written directly after the
    // name, its id and its value's id first.
    for mode in MODES {
        let text = write_in(ART_005, "a Resource with a location", &doc, mode);
        assert_eq!(
            lines_from(ART_005, &text, "x1", 5),
            [
                "<artifactElement xmi:type=\"sacm:Resource\" xmi:id=\"x1\" gid=\"G-1\" \
                 isCitation=\"true\" citedElement=\"x3\" abstractForm=\"x2\">",
                "<name lang=\"en\" content=\"X1\" />",
                "<location xmi:id=\"l\">",
                "<value xmi:id=\"l_en\" lang=\"en\" content=\"https://example.org/r.pdf\" />",
                "</location>",
            ],
            "{ART_005}: the location is not written directly after the name in {mode:?} mode\n\
             {text}"
        );
    }
    {
        let x1 = element(ART_005, &doc, "x1");
        assert_eq!(
            (
                x1.location(),
                x1.location_id().map(ElementId::as_str),
                x1.location_value_ids(),
                element(ART_005, &doc, "x2").location()
            ),
            (
                Some(&MultiLangString::from(en("https://example.org/r.pdf"))),
                Some("l"),
                [Some(ElementId::from("l_en"))].as_slice(),
                None
            ),
            "{ART_005}: the location of x1, and none on x2"
        );
    }
}

#[test]
fn sacm23_art_006_an_activity_with_every_feature_reads_in_both_modes_and_writes_back() {
    let doc = assert_every_feature_round_trips(
        ART_006,
        "Activity",
        ElementKind::Activity,
        " startTime=\"2024-02-12\" endTime=\"2024-02-29T17:30:00Z\"",
        "",
    );
    {
        let x1 = element(ART_006, &doc, "x1");
        assert_eq!(
            (x1.start_time(), x1.end_time(), x1.date()),
            (Some("2024-02-12"), Some("2024-02-29T17:30:00Z"), None),
            "{ART_006}: the times of x1"
        );
    }
    let _ = doc;
}

#[test]
fn sacm23_art_007_an_event_with_every_feature_reads_in_both_modes_and_writes_back() {
    let doc = assert_every_feature_round_trips(
        ART_007,
        "Event",
        ElementKind::Event,
        " date=\"2024-03-05\"",
        "",
    );
    {
        let x1 = element(ART_007, &doc, "x1");
        assert_eq!(
            (x1.date(), x1.version(), x1.start_time()),
            (Some("2024-03-05"), None, None),
            "{ART_007}: the date of x1"
        );
    }
    let _ = doc;
}

// --- SACM23-ART-004: version, and the worked example ----------------------------------

#[test]
#[expect(
    clippy::too_many_lines,
    reason = "the two forms, xmi:version, a repeat and the other kinds read best together"
)]
fn sacm23_art_004_version_reads_verbatim_in_both_forms_and_is_not_xmi_version() {
    for value in ["1.2", "", " v2 ", "2024-03-01"] {
        let what = format!("version={value:?}");
        let attribute = read_ok(
            ART_004,
            &what,
            &host_with("Artifact", &format!(" version=\"{value}\""), ""),
        );
        let element_form = read_ok(
            ART_004,
            &format!("<version>{value}</version>"),
            &host_with("Artifact", "", &format!("<version>{value}</version>")),
        );
        assert_eq!(
            (
                element(ART_004, &attribute, "h").version(),
                element(ART_004, &element_form, "h").version()
            ),
            (Some(value), Some(value)),
            "{ART_004}: {what}: not held verbatim in both forms"
        );
        assert!(
            attribute == element_form,
            "{ART_004}: {what}: the forms read differently"
        );
        let text = write_in(ART_004, &what, &element_form, Mode::Strict);
        assert!(
            line_with(ART_004, &text, "h").ends_with(&format!(" version=\"{value}\">")),
            "{ART_004}: {what}: not written as an attribute\n{text}"
        );
    }
    // Beside xmi:version on the root: the unqualified attribute is the
    // version, and the XMI one is SACM-XMI-011 Info.
    let input = host_with("Artifact", " version=\"1.2\"", "").replacen(
        "xmi:id=\"a\">",
        "xmi:id=\"a\" xmi:version=\"2.0\">",
        1,
    );
    let doc = read_reporting(
        ART_004,
        "xmi:version on the root",
        &input,
        Some(Code::XMI_VERSION_IGNORED),
    );
    assert_eq!(
        element(ART_004, &doc, "h").version(),
        Some("1.2"),
        "{ART_004}: xmi:version is taken for the version"
    );
    for mode in MODES {
        let output = match read(mode, &input) {
            Ok(output) => output,
            Err(error) => panic!("{ART_004}: xmi:version on the root: {error:?}"),
        };
        assert!(
            output
                .diagnostics()
                .iter()
                .all(|found| found.severity() == Severity::Info),
            "{ART_004}: xmi:version is not Info in {mode:?} mode: {:?}",
            output.diagnostics()
        );
    }
    // Twice: RepeatedValue in strict mode, the first kept in tolerant mode.
    let (input, marks) = marked(
        ART_004,
        &host_with(
            "Artifact",
            &format!(" {MARK}version=\"1.2\""),
            &format!("{MARK}<version>1.3</version>"),
        ),
    );
    let Err(error) = read(Mode::Strict, &input) else {
        panic!("{ART_004}: a version given twice was read strictly");
    };
    assert!(
        matches!(
            &error,
            ReadError::RepeatedValue { element, property, locations, .. }
                if element.as_str() == "h" && property == "version" && *locations == marks
        ),
        "{ART_004}: a version given twice: not RepeatedValue naming both: {error:?}"
    );
    let output = match read(Mode::Tolerant, &input) {
        Ok(output) => output,
        Err(error) => panic!("{ART_004}: a version given twice, tolerant: {error:?}"),
    };
    let found: Vec<_> = output
        .diagnostics()
        .iter()
        .map(|found| (found.code(), found.severity(), found.location()))
        .collect();
    assert_eq!(
        (found, element(ART_004, output.document(), "h").version()),
        (
            vec![(
                Code::MULTIPLICITY_VIOLATION,
                Severity::Warning,
                Some(marks[1])
            )],
            Some("1.2")
        ),
        "{ART_004}: a version given twice, tolerant: not one Warning keeping the first"
    );
    // On a kind without it: preserved.
    for class in ["Technique", "Event", "Resource", "Activity"] {
        for (form, attributes, content) in [
            (
                "the attribute",
                format!(" {MARK}version=\"1.2\""),
                String::new(),
            ),
            (
                "the value element",
                String::new(),
                format!("{MARK}<version>1.2</version>"),
            ),
        ] {
            let what = format!("{form} version on a {class}");
            let doc = assert_preserved_at(
                ART_004,
                &what,
                &host_with(class, &attributes, &content),
                "h",
            );
            assert_eq!(
                element(ART_004, &doc, "h").version(),
                None,
                "{ART_004}: {what}: read as a version"
            );
        }
    }
}

#[test]
fn sacm23_art_004_the_artifact_assets_example_reads_validates_and_writes_its_golden() {
    let doc = read_ok(ART_004, "the fixture", &assets_fixture(ART_004));
    assert_eq!(
        (
            placed(ART_004, &doc, "report"),
            placed(ART_004, &doc, "report_passed"),
            placed(ART_004, &doc, "released"),
            placed(ART_004, &doc, "reviewed"),
            placed(ART_004, &doc, "report_pdf"),
            placed(ART_004, &doc, "testing"),
            children(ART_004, &doc, "evidence"),
            children(ART_004, &doc, "report_pdf")
        ),
        (
            (ElementKind::Artifact, Some("evidence".to_owned())),
            (ElementKind::Property, Some("report".to_owned())),
            (ElementKind::Event, Some("evidence".to_owned())),
            (ElementKind::Event, Some("evidence".to_owned())),
            (ElementKind::Resource, Some("evidence".to_owned())),
            (ElementKind::Activity, Some("evidence".to_owned())),
            ids(&["report", "released", "reviewed", "report_pdf", "testing"]),
            ids(&["report_pdf_d"])
        ),
        "{ART_004}: the fixture's elements"
    );
    assert_eq!(
        validation::validate(&doc),
        [],
        "{ART_004}: the fixture has findings"
    );
    let golden = std::fs::read_to_string(ASSETS_GOLDEN)
        .unwrap_or_else(|error| panic!("{ART_004}: cannot read {ASSETS_GOLDEN}: {error}"));
    for mode in MODES {
        assert_eq!(
            write_in(ART_004, "the fixture", &doc, mode),
            golden,
            "{ART_004}: the fixture does not write the golden file's bytes in {mode:?} mode"
        );
    }
    let back = read_ok(ART_004, "the golden file", &golden);
    assert_eq!(
        semantic_compare(&doc, &back),
        [],
        "{ART_004}: the golden file does not read back equal to the fixture"
    );
    assert!(back == doc, "{ART_004}: the golden file reads back !=");
    {
        let get = |id: &str| element(ART_004, &doc, id);
        assert_eq!(
            (
                (get("report").version(), get("report").date()),
                get("released").date(),
                get("reviewed").date(),
                (get("testing").start_time(), get("testing").end_time()),
                get("report_pdf").location().map(MultiLangString::len),
                get("report_pdf").location_id().map(ElementId::as_str),
                get("report_pdf").location_value_ids(),
            ),
            (
                (Some("1.2"), Some("2024-03-01")),
                Some("2024-03-05T14:00:00+01:00"),
                Some("spring 2024"),
                (Some("2024-02-12"), Some("2024-02-29T17:30:00Z")),
                Some(2),
                Some("report_pdf_location"),
                [None, None].as_slice(),
            ),
            "{ART_004}: the fixture's values"
        );
    }
}

// --- SACM23-ART-005: location ----------------------------------------------------------

#[test]
fn sacm23_art_005_location_keeps_order_duplicates_and_absent_apart_from_empty() {
    // No location: None, and nothing is written.
    let doc = read_ok(ART_005, "no location", &host("Resource", ""));
    let h = element(ART_005, &doc, "h");
    assert_eq!(
        (h.location(), h.location_id(), h.location_value_ids()),
        (None, None, [].as_slice()),
        "{ART_005}: no location is not None"
    );
    let text = round_trip(ART_005, "no location", &doc, Mode::Strict);
    assert!(
        !text.contains("<location"),
        "{ART_005}: no location writes one\n{text}"
    );
    // Empty in every form: Some and empty, written as one empty element.
    for form in [
        "<location/>",
        "<location></location>",
        "<location> \n   </location>",
        "<location><!--c--></location>",
        "<location xmi:type=\"sacm:MultiLangString\"/>",
    ] {
        let doc = read_ok(ART_005, form, &host("Resource", form));
        assert_eq!(
            element(ART_005, &doc, "h").location(),
            Some(&MultiLangString::default()),
            "{ART_005}: {form}: not Some and empty"
        );
        for mode in MODES {
            let text = round_trip(ART_005, form, &doc, mode);
            assert_eq!(
                lines_from(ART_005, &text, "h", 3)[2],
                "<location />",
                "{ART_005}: {form}: not written as <location /> after the name in {mode:?} mode\n\
                 {text}"
            );
        }
    }
    // Values in order, a repeat kept.
    let what = "[en:a, sv:b, en:a]";
    let doc = read_ok(
        ART_005,
        what,
        &host(
            "Resource",
            "<location><value lang=\"en\" content=\"a\"/><value lang=\"sv\" content=\"b\"/>\
             <value lang=\"en\" content=\"a\"/></location>",
        ),
    );
    let expected: MultiLangString = [en("a"), LangString::new("b").with_lang("sv"), en("a")]
        .into_iter()
        .collect();
    assert_eq!(
        element(ART_005, &doc, "h").location(),
        Some(&expected),
        "{ART_005}: {what}: not three values in order"
    );
    round_trip(ART_005, what, &doc, Mode::Tolerant);
    // The four absent and empty states of a value's lang and content.
    let what = "the four lang and content states";
    let doc = read_ok(
        ART_005,
        what,
        &host(
            "Resource",
            "<location><value/><value lang=\"\"/><value content=\"\"/>\
             <value lang=\"\" content=\"\"/></location>",
        ),
    );
    let expected: MultiLangString = [
        LangString::default(),
        LangString::default().with_lang(""),
        LangString::new(""),
        LangString::new("").with_lang(""),
    ]
    .into_iter()
    .collect();
    assert_eq!(
        element(ART_005, &doc, "h").location(),
        Some(&expected),
        "{ART_005}: {what}: not kept apart"
    );
    for mode in MODES {
        round_trip(ART_005, what, &doc, mode);
    }
}

#[test]
#[expect(
    clippy::too_many_lines,
    reason = "the ids' reading, writing, comparison and id space read best as one list"
)]
fn sacm23_art_005_location_ids_are_kept_by_position_written_first_and_compared() {
    let location = |location_id: &str, third: &str| {
        host(
            "Resource",
            &format!(
                "<location{location_id}><value lang=\"en\" content=\"a\" xmi:id=\"v1\"/>\
                 <value content=\"b\"/><value{third} content=\"c\"/></location>"
            ),
        )
    };
    let doc = read_ok(
        ART_005,
        "ids on a location",
        &location(" xmi:id=\"l\"", " xmi:id=\"v3\""),
    );
    let h = element(ART_005, &doc, "h");
    assert_eq!(
        (
            h.location_id().map(ElementId::as_str),
            h.location_value_ids()
        ),
        (
            Some("l"),
            [
                Some(ElementId::from("v1")),
                None,
                Some(ElementId::from("v3"))
            ]
            .as_slice()
        ),
        "{ART_005}: the ids are not kept by position"
    );
    for mode in MODES {
        let text = round_trip(ART_005, "ids on a location", &doc, mode);
        assert_eq!(
            lines_from(ART_005, &text, "h", 7)[2..],
            [
                "<location xmi:id=\"l\">",
                "<value xmi:id=\"v1\" lang=\"en\" content=\"a\" />",
                "<value content=\"b\" />",
                "<value xmi:id=\"v3\" content=\"c\" />",
                "</location>",
            ],
            "{ART_005}: the ids are not written first in {mode:?} mode\n{text}"
        );
    }
    // Compared: LocationId and LocationValueIds.
    let other_id = read_ok(
        ART_005,
        "another location id",
        &location(" xmi:id=\"l2\"", " xmi:id=\"v3\""),
    );
    let found = semantic_compare(&doc, &other_id);
    assert_eq!(
        found,
        [Difference::LocationId {
            id: ElementId::from("h"),
            left: Some(ElementId::from("l")),
            right: Some(ElementId::from("l2")),
        }],
        "{ART_005}: another location id is not one LocationId difference"
    );
    assert_eq!(
        found[0].to_string(),
        "element \"h\" differs in the xmi:id of its location: \"l\" on the left, \"l2\" on the \
         right",
        "{ART_005}: the Display of LocationId"
    );
    let fewer_ids = read_ok(
        ART_005,
        "a value without its id",
        &location(" xmi:id=\"l\"", ""),
    );
    let found = semantic_compare(&doc, &fewer_ids);
    assert_eq!(
        found,
        [Difference::LocationValueIds {
            id: ElementId::from("h"),
            left: vec![
                Some(ElementId::from("v1")),
                None,
                Some(ElementId::from("v3"))
            ],
            right: vec![Some(ElementId::from("v1")), None, None],
        }],
        "{ART_005}: a value without its id is not one LocationValueIds difference"
    );
    assert_eq!(
        found[0].to_string(),
        "element \"h\" differs in the xmi:ids of its location's values: [\"v1\", no id, \"v3\"] \
         on the left, [\"v1\", no id, no id] on the right",
        "{ART_005}: the Display of LocationValueIds"
    );
    // The ids are in the document's id space: a repeat is DuplicateId at
    // both start tags, in both modes.
    for (what, template, repeated) in [
        (
            "the location id is the Resource's",
            in_a(&format!(
                "{MARK}<artifactElement xmi:type=\"sacm:Resource\" xmi:id=\"h\">\
                 <name content=\"H\"/>{MARK}<location xmi:id=\"h\"/></artifactElement>"
            )),
            "h",
        ),
        (
            "a value id is a later Technique's",
            in_a(&format!(
                "<artifactElement xmi:type=\"sacm:Resource\" xmi:id=\"h\"><name content=\"H\"/>\
                 <location>{MARK}<value xmi:id=\"t9\" content=\"x\"/></location></artifactElement>\
                 {MARK}<artifactElement xmi:type=\"sacm:Technique\" xmi:id=\"t9\">\
                 <name content=\"T\"/></artifactElement>"
            )),
            "t9",
        ),
        (
            "a value id is the location id",
            host(
                "Resource",
                &format!("{MARK}<location xmi:id=\"w\">{MARK}<value xmi:id=\"w\"/></location>"),
            ),
            "w",
        ),
    ] {
        let (input, marks) = marked(ART_005, &template);
        for mode in MODES {
            let Err(error) = read(mode, &input) else {
                panic!("{ART_005}: {what}: read in {mode:?} mode");
            };
            assert!(
                matches!(&error, ReadError::DuplicateId { id, .. } if id.as_str() == repeated),
                "{ART_005}: {what}: not DuplicateId for {repeated:?} in {mode:?} mode: {error:?}"
            );
            assert_eq!(
                (error.code(), error.locations()),
                (Some(Code::ID_DUPLICATE), marks.as_slice()),
                "{ART_005}: {what}: not SACM-ID-001 at both start tags in {mode:?} mode"
            );
        }
    }
    // Generated ids skip them, and a caller id equal to one is DuplicateId.
    let mut doc = read_ok(
        ART_005,
        "location ids that look generated",
        &host(
            "Resource",
            "<location xmi:id=\"technique_1\"><value xmi:id=\"technique_2\" content=\"x\"/>\
             </location>",
        ),
    );
    let mutation = applied(ART_005, &mut doc, CreateTechnique::new("a", named("T")));
    assert_eq!(
        mutation.created_ids(),
        [ElementId::from("technique_3")],
        "{ART_005}: a generated id is a location or value id"
    );
    for taken in ["technique_1", "technique_2"] {
        let error = refused_edit(
            ART_005,
            &mut doc,
            CreateTechnique::new("a", named("T")).with_id(taken),
        );
        assert!(
            matches!(&error, EditError::DuplicateId { id, .. } if id.as_str() == taken),
            "{ART_005}: a caller id equal to the location id {taken:?}: {error:?}"
        );
    }
}

#[test]
fn sacm23_art_005_a_second_location_is_a_repeated_value() {
    let (input, marks) = marked(
        ART_005,
        &host(
            "Resource",
            &format!(
                "{MARK}<location><value content=\"a\"/></location>\n{MARK}<location>\
                 <value content=\"b\"/></location>"
            ),
        ),
    );
    let Err(error) = read(Mode::Strict, &input) else {
        panic!("{ART_005}: two locations were read strictly");
    };
    assert!(
        matches!(
            &error,
            ReadError::RepeatedValue { element, property, locations, .. }
                if element.as_str() == "h" && property == "location" && *locations == marks
        ),
        "{ART_005}: two locations: not RepeatedValue {{ property: \"location\" }} naming both: \
         {error:?}"
    );
    assert_eq!(
        (error.code(), error.location()),
        (Some(Code::MULTIPLICITY_VIOLATION), Some(marks[1])),
        "{ART_005}: two locations: not SACM-MULT-001 with location() the second"
    );
    let output = match read(Mode::Tolerant, &input) {
        Ok(output) => output,
        Err(error) => panic!("{ART_005}: two locations, tolerant: {error:?}"),
    };
    let found: Vec<_> = output
        .diagnostics()
        .iter()
        .map(|found| (found.code(), found.severity(), found.location()))
        .collect();
    assert_eq!(
        found,
        [(
            Code::MULTIPLICITY_VIOLATION,
            Severity::Warning,
            Some(marks[1])
        )],
        "{ART_005}: two locations, tolerant: not one SACM-MULT-001 Warning at the second"
    );
    let h = element(ART_005, output.document(), "h");
    assert!(
        h.has_preserved_content(),
        "{ART_005}: two locations, tolerant: the second is not preserved"
    );
    assert_eq!(
        h.location(),
        Some(&MultiLangString::from(LangString::new("a"))),
        "{ART_005}: two locations, tolerant: not the first"
    );
}

#[test]
fn sacm23_art_005_a_malformed_or_misplaced_location_is_preserved_whole() {
    let mut cases: Vec<(String, String)> = [
        (
            "typed sacm:LangString",
            "<location xmi:type=\"sacm:LangString\"><value content=\"x\"/></location>",
        ),
        (
            "typed sacm:Bogus",
            "<location xmi:type=\"sacm:Bogus\"><value content=\"x\"/></location>",
        ),
        (
            "a foo attribute",
            "<location foo=\"1\"><value content=\"x\"/></location>",
        ),
        (
            "text beside a value",
            "<location><value content=\"x\"/> text</location>",
        ),
        (
            "the single-language shorthand as attributes",
            "<location lang=\"en\" content=\"x\"/>",
        ),
        ("text and no value", "<location>x</location>"),
        (
            "the sacm prefix",
            "<sacm:location><value content=\"x\"/></sacm:location>",
        ),
        (
            "another child",
            "<location><value content=\"x\"/><foo/></location>",
        ),
        (
            "a foo attribute on a value",
            "<location><value foo=\"1\" content=\"x\"/></location>",
        ),
    ]
    .iter()
    .map(|(what, location)| {
        (
            format!("a location with {what} on a Resource"),
            host("Resource", &format!("{MARK}{location}")),
        )
    })
    .collect();
    for class in ["Technique", "Artifact", "Event", "Activity", "Participant"] {
        cases.push((
            format!("a location on a {class}"),
            host(
                class,
                &format!("{MARK}<location><value content=\"x\"/></location>"),
            ),
        ));
    }
    for (what, template) in &cases {
        let doc = assert_preserved_at(ART_005, what, template, "h");
        assert_eq!(
            element(ART_005, &doc, "h").location(),
            None,
            "{ART_005}: {what}: read as a location"
        );
        let _ = doc;
    }
}

// --- SACM23-ART-006: the times are not ordered ---------------------------------------

#[test]
fn sacm23_art_006_an_end_time_before_its_start_time_is_held_without_a_finding() {
    for (what, attributes, start, end) in [
        (
            "an endTime before its startTime",
            " startTime=\"2024-03-01\" endTime=\"2024-02-01\"",
            Some("2024-03-01"),
            Some("2024-02-01"),
        ),
        (
            "an endTime a year before, in another form",
            " startTime=\"2024-03-01T00:00:00Z\" endTime=\"2023\"",
            Some("2024-03-01T00:00:00Z"),
            Some("2023"),
        ),
        (
            "a startTime alone",
            " startTime=\"2024-03-01\"",
            Some("2024-03-01"),
            None,
        ),
        (
            "an endTime alone",
            " endTime=\"2024-02-01\"",
            None,
            Some("2024-02-01"),
        ),
    ] {
        let doc = read_ok(ART_006, what, &host_with("Activity", attributes, ""));
        assert_eq!(
            validation::validate(&doc),
            [],
            "{ART_006}: {what}: validate reports the times"
        );
        for mode in MODES {
            let text = round_trip(ART_006, what, &doc, mode);
            assert!(
                line_with(ART_006, &text, "h").ends_with(&format!("{attributes}>")),
                "{ART_006}: {what}: not written as given in {mode:?} mode\n{text}"
            );
        }
        assert_eq!(
            (
                element(ART_006, &doc, "h").start_time(),
                element(ART_006, &doc, "h").end_time()
            ),
            (start, end),
            "{ART_006}: {what}: not held"
        );
        let _ = (start, end);
    }
}

// --- SACM23-ART-007: occurece ---------------------------------------------------------

/// The code, severity, location and affected ids of each diagnostic.
fn reported(found: &[Diagnostic]) -> Vec<(Code, Severity, Option<SourceLocation>, Vec<ElementId>)> {
    found
        .iter()
        .map(|found| {
            (
                found.code(),
                found.severity(),
                found.location(),
                found.affected().to_vec(),
            )
        })
        .collect()
}

#[test]
#[expect(
    clippy::too_many_lines,
    reason = "the strict keep, the tolerant read and the other kinds read best together"
)]
fn sacm23_art_007_occurece_on_an_event_is_read_tolerantly_and_kept_strictly_by_sacm23_xmi_009() {
    // Since slice 0057 (SACM23-XMI-009, ADR 0034 amendment 2) the model's
    // spelling of an Event's `date` is read by a tolerant read, unless a
    // `date` is given, which is read instead; a strict read keeps it.
    // (what, attributes, content, the date beside it)
    for (what, attributes, content, beside) in [
        (
            "occurece as an attribute",
            format!(" {MARK}occurece=\"2024-03-05\""),
            String::new(),
            None,
        ),
        (
            "occurece as a value element",
            String::new(),
            format!("{MARK}<occurece>2024-03-05</occurece>"),
            None,
        ),
        (
            "occurece beside date",
            format!(" date=\"2024-03-06\" {MARK}occurece=\"2024-03-05\""),
            String::new(),
            Some("2024-03-06"),
        ),
        (
            "<occurece> after <date>",
            String::new(),
            format!("<date>2024-03-06</date>{MARK}<occurece>2024-03-05</occurece>"),
            Some("2024-03-06"),
        ),
    ] {
        let (input, at) = one_mark(ART_007, &host_with("Event", &attributes, &content));
        // Strict: kept at it as preserved content of the Event, with one
        // SACM-XMI-013 Error there naming SACM23-XMI-009, and never taken for
        // the date.
        let output = match read(Mode::Strict, &input) {
            Ok(output) => output,
            Err(error) => panic!("{ART_007}: {what}: a strict read refused it: {error:?}"),
        };
        assert_eq!(
            reported(output.diagnostics()),
            [(
                Code::XMI_SPELLING_VARIANT,
                Severity::Error,
                Some(at),
                list(&["h"])
            )],
            "{ART_007}: {what}: not one SACM-XMI-013 Error at it affecting the Event"
        );
        let message = output.diagnostics()[0].message();
        assert!(
            output.diagnostics()[0].requirement_id() == "SACM23-XMI-009"
                && message.contains("occurece")
                && !message.contains('\n'),
            "{ART_007}: {what}: the Error does not name occurece and SACM23-XMI-009: {message:?}"
        );
        let doc = output.into_document();
        assert_eq!(
            (
                element(ART_007, &doc, "h").kind(),
                element(ART_007, &doc, "h").has_preserved_content(),
                element(ART_007, &doc, "h").date()
            ),
            (ElementKind::Event, true, beside),
            "{ART_007}: {what}: not an Event holding it as preserved content, or it is taken \
             for the date"
        );
        let text = write_in(ART_007, what, &doc, Mode::Tolerant);
        assert!(
            text.contains("occurece") && text.contains("2024-03-05"),
            "{ART_007}: {what}: a tolerant write does not write it back\n{text}"
        );
        let back = match read(Mode::Strict, &text) {
            Ok(output) => output.into_document(),
            Err(error) => panic!("{ART_007}: {what}: the tolerant output does not read: {error}"),
        };
        assert!(
            back == doc,
            "{ART_007}: {what}: the tolerant output reads back strictly !=: {:?}",
            semantic_compare(&doc, &back)
        );
        match xmi::write(&doc, &WriteOptions::default().with_mode(Mode::Strict)) {
            Err(WriteError::PreservedContent { elements, .. }) if elements == list(&["h"]) => {}
            other => {
                panic!("{ART_007}: {what}: a strict write is not PreservedContent [h]: {other:?}")
            }
        }
        // Tolerant: read as the date with one SACM-XMI-013 Warning at it; or,
        // beside a date, kept as the repeat with one SACM-MULT-001 Warning.
        let output = match read(Mode::Tolerant, &input) {
            Ok(output) => output,
            Err(error) => panic!("{ART_007}: {what}: a tolerant read refused it: {error:?}"),
        };
        let code = if beside.is_some() {
            Code::MULTIPLICITY_VIOLATION
        } else {
            Code::XMI_SPELLING_VARIANT
        };
        assert_eq!(
            reported(output.diagnostics()),
            [(code, Severity::Warning, Some(at), list(&["h"]))],
            "{ART_007}: {what}: not one {code} Warning at it affecting the Event"
        );
        let doc = output.into_document();
        assert_eq!(
            (
                element(ART_007, &doc, "h").has_preserved_content(),
                element(ART_007, &doc, "h").date()
            ),
            (beside.is_some(), beside.or(Some("2024-03-05"))),
            "{ART_007}: {what}: the date read, or what is kept"
        );
        let text = write_in(ART_007, what, &doc, Mode::Tolerant);
        let back = match read(Mode::Tolerant, &text) {
            Ok(output) => output.into_document(),
            Err(error) => panic!("{ART_007}: {what}: the tolerant output does not read: {error}"),
        };
        assert!(
            back == doc,
            "{ART_007}: {what}: the tolerant output reads back !=: {:?}",
            semantic_compare(&doc, &back)
        );
        if beside.is_none() {
            let text = write_in(ART_007, what, &doc, Mode::Strict);
            assert!(
                text.contains(" date=\"2024-03-05\"") && !text.contains("occurece"),
                "{ART_007}: {what}: a strict write does not write the date\n{text}"
            );
        }
    }
    // On another kind, `occurece` is an ordinary unknown attribute or child.
    for class in ["Artifact", "Technique", "Activity"] {
        for (form, attributes, content) in [
            (
                "attribute",
                format!(" {MARK}occurece=\"2024-03-05\""),
                String::new(),
            ),
            (
                "value element",
                String::new(),
                format!("{MARK}<occurece>2024-03-05</occurece>"),
            ),
        ] {
            let what = format!("the occurece {form} on an {class}");
            assert_preserved_at(
                ART_007,
                &what,
                &host_with(class, &attributes, &content),
                "h",
            );
        }
    }
}

// --- The existing edits apply (item 6 of SACM23-ART-004, the last of the others) -------

#[test]
fn sacm23_art_004_set_name_gid_flag_references_reorder_and_delete_apply_to_an_artifact() {
    assert_the_existing_edits_apply(ART_004, "Artifact");
}

#[test]
fn sacm23_art_005_set_name_gid_flag_references_reorder_and_delete_apply_to_a_resource() {
    assert_the_existing_edits_apply(ART_005, "Resource");
}

#[test]
fn sacm23_art_006_set_name_gid_flag_references_reorder_and_delete_apply_to_an_activity() {
    assert_the_existing_edits_apply(ART_006, "Activity");
}

#[test]
fn sacm23_art_007_set_name_gid_flag_references_reorder_and_delete_apply_to_an_event() {
    assert_the_existing_edits_apply(ART_007, "Event");
}
