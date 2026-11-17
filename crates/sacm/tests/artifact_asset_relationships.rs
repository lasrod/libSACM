//! Tests for `ArtifactAssetRelationship` (SACM23-ART-010; formal/23-05-08
//! §12.14 (pp. 49-50) "`ArtifactAssetRelationship` enables the `ArtifactAssets` of
//! a structured assurance case to be linked together.", "Superclass
//! `ArtifactAsset`", "source:ArtifactAsset[1..*]", "target:ArtifactAsset[1..*]";
//! §12.6 `property:Property[0..*] (composition)`; ptc/22-03-13
//! `Artifact::ArtifactAssertedRelationship`; D-TEXT-MODEL, D-SEV; ADR 0026,
//! ADR 0028, ADR 0034 amendment 2; the maintainer's M3 decisions 5 and 7).
//!
//! An `ArtifactAssetRelationship` is read and written typed in an
//! ArtifactPackage's `artifactElement`, with the Base features of every
//! `ArtifactAsset`, its `property` children and the two reference ends `source`
//! and `target`, each `[1..*]` and typed `ArtifactAsset`. The ends are
//! `ReferenceEnd::ArtifactSource` and `ReferenceEnd::ArtifactTarget` (XMI
//! names `source` and `target`), apart from an `AssertedRelationship`'s
//! `Source` and `Target`. `SacmElement::sources()` and `targets()` answer for
//! both relationship classes. The ends are validated (SACM-MULT-001 for an
//! empty end, SACM-REF-001, SACM-REF-002, and a repeat), edited, compared,
//! and handled by the delete policies; `CreateArtifactAssetRelationship`
//! creates one.
//!
//! Expected locations are marked in the input with [`MARK`] (U+2038), which
//! the test removes before reading. Inputs end lines with LF only.

use std::collections::BTreeSet;

use sacm::compare::{Difference, semantic_compare};
use sacm::diagnostic::{Code, Diagnostic, Severity, SourceLocation};
use sacm::edit::{
    self, CopyElements, CreateProperty, CrossPackageReferencePolicy, DeleteElement, EditError,
    MoveElement, Mutation, Operation, PackageDeletePolicy, ReferenceDeletePolicy, ReorderElements,
    SetFlag, SetGid, SetName, SetReferences,
};
use sacm::edit::{AddReference, CreateArtifactAssetRelationship, RemoveReference};
use sacm::model::{Document, ElementId, ElementKind, Flag, LangString, ReferenceEnd, SacmElement};
use sacm::validation;
use sacm::xmi::{self, Mode, ReadError, ReadOptions, ReadOutput, WriteError, WriteOptions};

const ROW: &str = "SACM23-ART-010";

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

/// The worked fixture of slice 0057.
const FIXTURE: &str = concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/tests/data/sacm23/xmi/artifact-asset-relationship-valid.sacm.xmi"
);

/// The exact output of writing [`FIXTURE`], in either mode.
const GOLDEN: &str = concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/tests/data/sacm23/xmi/golden/artifact-asset-relationship.golden.sacm.xmi"
);

/// The start of the written start tag of the relationship `rel`.
const REL_TAG: &str = "<artifactElement xmi:type=\"sacm:ArtifactAssetRelationship\" xmi:id=\"rel\"";

// --- Inputs ------------------------------------------------------------------------

/// A typed `artifactElement` child `id` of class `class`, named `id`.
fn asset(class: &str, id: &str) -> String {
    format!(
        "<artifactElement xmi:type=\"sacm:{class}\" xmi:id=\"{id}\"><name content=\"{id}\"/>\
         </artifactElement>"
    )
}

/// The `ArtifactAssetRelationship` `id` with `attributes`, named `id`, holding
/// `content` after its name.
fn rel(id: &str, attributes: &str, content: &str) -> String {
    format!(
        "<artifactElement xmi:type=\"sacm:ArtifactAssetRelationship\" xmi:id=\"{id}\"{attributes}>\
         <name content=\"{id}\"/>{content}</artifactElement>"
    )
}

/// The AssuranceCasePackage root `case`, one element per line:
/// - the ArgumentPackage `p`: the Claims `c1` and `c2`, the
///   `ArtifactReference` `ar1` and the `AssertedInference` `i1` (source `c2`,
///   target `c1`);
/// - the ArtifactPackage `evidence`: the Artifact `report` (holding the
///   Property `pr1`), the Activity `testing`, the Participant `tester`, the
///   Event `released`, the Resource `pdf`, the Technique `fta` and the
///   `ArtifactGroup` `grp`, then `body`;
/// - the ArtifactPackage `other`, holding `other`.
fn case(body: &str, other: &str) -> String {
    let report = "<artifactElement xmi:type=\"sacm:Artifact\" xmi:id=\"report\">\
                  <name content=\"report\"/><property xmi:id=\"pr1\"><name content=\"pr1\"/>\
                  </property></artifactElement>";
    format!(
        r#"<sacm:AssuranceCasePackage{NS} xmi:id="case">
  <name content="Case"/>
  <argumentPackage xmi:id="p">
    <name content="P"/>
    <argumentElement xmi:type="sacm:Claim" xmi:id="c1"><name content="C1"/></argumentElement>
    <argumentElement xmi:type="sacm:Claim" xmi:id="c2"><name content="C2"/></argumentElement>
    <argumentElement xmi:type="sacm:ArtifactReference" xmi:id="ar1"><name content="AR1"/></argumentElement>
    <argumentElement xmi:type="sacm:AssertedInference" xmi:id="i1" source="c2" target="c1"><name content="I1"/></argumentElement>
  </argumentPackage>
  <artifactPackage xmi:id="evidence">
    <name content="Evidence"/>
    {report}
    {}
    {}
    {}
    {}
    {}
    {}
    {body}
  </artifactPackage>
  <artifactPackage xmi:id="other">
    <name content="Other"/>
    {other}
  </artifactPackage>
</sacm:AssuranceCasePackage>"#,
        asset("Activity", "testing"),
        asset("Participant", "tester"),
        asset("Event", "released"),
        asset("Resource", "pdf"),
        asset("Technique", "fta"),
        asset("ArtifactGroup", "grp"),
    )
}

/// [`case`] with the relationship `rel` (source `report`, targets `testing`
/// and `tester`) in `evidence`, after `before`.
fn with_rel(before: &str) -> String {
    case(
        &format!(
            "{before}{}",
            rel("rel", " source=\"report\" target=\"testing tester\"", "")
        ),
        "",
    )
}

fn loc(line: u32, column: u32) -> SourceLocation {
    let Some(location) = SourceLocation::new(line, column) else {
        panic!("{ROW}: setup: SourceLocation::new({line}, {column}) refused a 1-based position");
    };
    location
}

/// `template` without its marks, and the location of each mark in order:
/// lines end at LF, and columns count characters.
fn marked(template: &str) -> (String, Vec<SourceLocation>) {
    let (mut line, mut column) = (1_u32, 1_u32);
    let mut text = String::with_capacity(template.len());
    let mut marks = Vec::new();
    for c in template.chars() {
        if c == MARK {
            marks.push(loc(line, column));
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

// --- Reading, writing and validating --------------------------------------------------

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
                assert!(
                    output.diagnostics().is_empty(),
                    "{ROW}: {what}: reading in {mode:?} mode reported {:?}",
                    output.diagnostics()
                );
                documents.push(output.into_document());
            }
            Err(error) => {
                panic!("{ROW}: {what}: a {mode:?} read refused {input:?}: {error:?} ({error})")
            }
        }
    }
    assert!(
        documents[0] == documents[1],
        "{ROW}: {what}: the modes read different documents"
    );
    documents.swap_remove(0)
}

/// Reads `input` in strict mode, which the test expects to refuse it.
fn refused(what: &str, input: &str) -> ReadError {
    match read(Mode::Strict, input) {
        Ok(output) => panic!(
            "{ROW}: {what}: a strict read returned a document of {} elements",
            output.document().element_count()
        ),
        Err(error) => error,
    }
}

/// `(code, severity, location, affected)` of each diagnostic.
type Reported = Vec<(Code, Severity, Option<SourceLocation>, Vec<ElementId>)>;

fn reported(found: &[Diagnostic]) -> Reported {
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

/// Asserts that `template`, without its one mark, reads in both modes and
/// keeps one construct as preserved content of `holder`: exactly one
/// SACM-XMI-003 at the mark, affecting `holder`, a Warning in tolerant mode
/// and an Error in strict mode, and the same document in both. Returns the
/// strict document.
fn assert_preserved_at(what: &str, template: &str, holder: &str) -> Document {
    let (input, marks) = marked(template);
    let [at] = marks.as_slice() else {
        panic!("{ROW}: setup: {what} has not one mark");
    };
    let mut documents = Vec::new();
    for (mode, severity) in [
        (Mode::Strict, Severity::Error),
        (Mode::Tolerant, Severity::Warning),
    ] {
        let output = match read(mode, &input) {
            Ok(output) => output,
            Err(error) => panic!(
                "{ROW}: {what}: a {mode:?} read refused it instead of preserving it: {error:?} \
                 ({error})"
            ),
        };
        assert_eq!(
            reported(output.diagnostics()),
            [(
                Code::XMI_UNKNOWN_CONTENT,
                severity,
                Some(*at),
                vec![ElementId::from(holder)]
            )],
            "{ROW}: {what}: not one SACM-XMI-003 at the construct affecting {holder:?} in \
             {mode:?} mode"
        );
        let doc = output.into_document();
        assert!(
            doc.get(holder)
                .is_some_and(SacmElement::has_preserved_content),
            "{ROW}: {what}: {holder:?} holds no preserved content in {mode:?} mode"
        );
        documents.push(doc);
    }
    assert!(
        documents[0] == documents[1],
        "{ROW}: {what}: the modes read different documents"
    );
    documents.swap_remove(0)
}

fn write_in(what: &str, doc: &Document, mode: Mode) -> String {
    match xmi::write(doc, &WriteOptions::default().with_mode(mode)) {
        Ok(text) => text,
        Err(error) => panic!("{ROW}: {what}: a {mode:?} write refused the document: {error:?}"),
    }
}

/// Writes `doc` in `mode`, reads the output back and writes that again: the
/// read-back must be `==` and the second write must give the same bytes.
fn round_trip(what: &str, doc: &Document, mode: Mode) -> String {
    let text = write_in(what, doc, mode);
    let back = read_ok(&format!("{what}, written in {mode:?} mode"), &text);
    assert_eq!(
        semantic_compare(doc, &back),
        [],
        "{ROW}: {what}: the output does not read back equal\n{text}"
    );
    assert!(back == *doc, "{ROW}: {what}: the output reads back !=");
    assert_eq!(
        write_in(what, &back, mode),
        text,
        "{ROW}: {what}: writing what was read back gives other bytes"
    );
    text
}

fn element<'d>(doc: &'d Document, id: &str) -> SacmElement<'d> {
    let Some(found) = doc.get(id) else {
        panic!("{ROW}: the document has no element {id:?}");
    };
    found
}

fn strs(values: &[ElementId]) -> Vec<&str> {
    values.iter().map(ElementId::as_str).collect()
}

fn list(values: &[&str]) -> Vec<ElementId> {
    values.iter().copied().map(ElementId::from).collect()
}

fn children(doc: &Document, id: &str) -> Vec<String> {
    element(doc, id)
        .children()
        .map(|child| child.id().as_str().to_owned())
        .collect()
}

fn line_with<'t>(text: &'t str, id: &str) -> &'t str {
    let attribute = format!("xmi:id=\"{id}\"");
    let Some(line) = text.lines().find(|line| line.contains(&attribute)) else {
        panic!("{ROW}: the output has no line with {attribute}:\n{text}");
    };
    line.trim()
}

/// `(code, severity, requirement, clause, location, affected)` of a finding.
type Finding = (
    Code,
    Severity,
    String,
    Option<String>,
    Option<SourceLocation>,
    Vec<ElementId>,
);

fn finding(found: &Diagnostic) -> Finding {
    (
        found.code(),
        found.severity(),
        found.requirement_id().to_owned(),
        found.clause().map(str::to_owned),
        found.location(),
        found.affected().to_vec(),
    )
}

fn findings(doc: &Document) -> Vec<Finding> {
    validation::validate(doc).iter().map(finding).collect()
}

/// A finding of `code` and `severity` for `requirement`, clause 12.14, no
/// location, affecting `affected`.
fn expected(code: Code, severity: Severity, requirement: &str, affected: &[&str]) -> Finding {
    (
        code,
        severity,
        requirement.to_owned(),
        Some("12.14".to_owned()),
        None,
        list(affected),
    )
}

// --- Edits -------------------------------------------------------------------------

/// Applies `operation`, which the test expects to succeed, after checking
/// that `edit::check` accepts it and `edit::preview` gives the mutation
/// `apply` gives.
fn applied(doc: &mut Document, operation: impl Into<Operation>) -> Mutation {
    let operation = operation.into();
    let checked = edit::check(doc, &operation);
    let previewed = edit::preview(doc, &operation).map(|preview| preview.mutation().clone());
    let mutation = match edit::apply(doc, operation.clone()) {
        Ok(mutation) => mutation,
        Err(error) => panic!("{ROW}: {operation:?} was refused: {error} ({error:?})"),
    };
    assert_eq!(
        checked,
        Ok(()),
        "{ROW}: check refuses {operation:?}, which apply accepts"
    );
    assert_eq!(
        previewed.as_ref(),
        Ok(&mutation),
        "{ROW}: preview of {operation:?} does not give apply's mutation"
    );
    mutation
}

/// Applies `operation`, which the test expects to be refused leaving `doc`
/// `==` to before, with `check` and `preview` refusing it the same, and
/// returns why.
fn refuse(doc: &mut Document, operation: impl Into<Operation>) -> EditError {
    let operation = operation.into();
    let before = doc.clone();
    let checked = edit::check(doc, &operation);
    let previewed = edit::preview(doc, &operation).map(|preview| preview.mutation().clone());
    let error = match edit::apply(doc, operation.clone()) {
        Ok(mutation) => panic!("{ROW}: {operation:?} was accepted: {mutation:?}"),
        Err(error) => error,
    };
    assert!(
        *doc == before,
        "{ROW}: the refused {operation:?} changed the document"
    );
    assert_eq!(
        (checked, previewed),
        (Err(error.clone()), Err(error.clone())),
        "{ROW}: check or preview answer {operation:?} otherwise than apply"
    );
    let message = error.to_string();
    assert!(
        !message.contains('\n') && message.starts_with(char::is_lowercase),
        "{ROW}: the message {message:?} is not one line starting in lower case"
    );
    error
}

/// What every successful delete keeps (LIB-EDIT-005's after-conditions,
/// D-ATOMIC): `created_ids` is empty; `deleted_ids` are the left-only ids of
/// `semantic_compare(before, after)`, in pre-order before; the survivors are
/// the elements before minus `deleted_ids`, in order; each surviving end, of
/// every end in `ReferenceEnd::ALL`, is its old list minus the deleted ids,
/// in order; and `changed_ids` are the survivors whose ends changed, in
/// pre-order after.
fn assert_clean_delete(what: &str, before: &Document, after: &Document, mutation: &Mutation) {
    assert_eq!(
        mutation.created_ids(),
        [] as [ElementId; 0],
        "{ROW}: {what}: a delete created ids"
    );
    let left_only: Vec<ElementId> = semantic_compare(before, after)
        .into_iter()
        .filter_map(|difference| match difference {
            Difference::LeftOnly { id, .. } => Some(id),
            _ => None,
        })
        .collect();
    assert_eq!(
        mutation.deleted_ids(),
        left_only,
        "{ROW}: {what}: deleted_ids() is not the left-only ids of semantic_compare"
    );
    let deleted: BTreeSet<&ElementId> = mutation.deleted_ids().iter().collect();
    let survivors: Vec<&ElementId> = before
        .elements()
        .map(SacmElement::id)
        .filter(|id| !deleted.contains(id))
        .collect();
    let now: Vec<&ElementId> = after.elements().map(SacmElement::id).collect();
    assert_eq!(
        now, survivors,
        "{ROW}: {what}: the survivors are not the elements before minus deleted_ids"
    );
    let mut changed = Vec::new();
    for survivor in after.elements() {
        let old = element(before, survivor.id().as_str());
        let mut lost = false;
        for &end in ReferenceEnd::ALL {
            let kept: Vec<ElementId> = old
                .references(end)
                .iter()
                .filter(|id| !deleted.contains(id))
                .cloned()
                .collect();
            assert_eq!(
                survivor.references(end),
                kept.as_slice(),
                "{ROW}: {what}: the {end:?} of {:?} is not its old list minus the deleted ids",
                survivor.id()
            );
            lost |= kept.len() != old.references(end).len();
        }
        if lost {
            changed.push(survivor.id().clone());
        }
    }
    assert_eq!(
        mutation.changed_ids(),
        changed,
        "{ROW}: {what}: changed_ids() are not the survivors whose ends changed"
    );
}

/// The sources and targets of `id`, as strings.
fn ends_of(doc: &Document, id: &str) -> (Vec<String>, Vec<String>) {
    let found = element(doc, id);
    let owned = |ids: &[ElementId]| -> Vec<String> {
        ids.iter().map(|id| id.as_str().to_owned()).collect()
    };
    (owned(found.sources()), owned(found.targets()))
}

fn owned(values: &[&str]) -> Vec<String> {
    values.iter().map(|&value| value.to_owned()).collect()
}

fn named(content: &str) -> LangString {
    LangString::new(content).with_lang("en")
}

// =====================================================================================
// Item 1: reading, the Base features, and placements
// =====================================================================================

#[test]
fn sacm23_art_010_reads_in_both_modes_with_its_sources_and_targets() {
    let what = "source=\"report\" target=\"testing tester\"";
    let doc = read_ok(what, &with_rel(""));
    let found = element(&doc, "rel");
    assert_eq!(
        (
            found.kind(),
            found.parent().map(|parent| parent.id().as_str().to_owned())
        ),
        (
            ElementKind::ArtifactAssetRelationship,
            Some("evidence".to_owned())
        ),
        "{ROW}: {what}: not an ArtifactAssetRelationship in evidence"
    );
    assert_eq!(
        ends_of(&doc, "rel"),
        (owned(&["report"]), owned(&["testing", "tester"])),
        "{ROW}: {what}: sources() and targets()"
    );
    assert_eq!(
        children(&doc, "evidence").last().map(String::as_str),
        Some("rel"),
        "{ROW}: {what}: the relationship is not the last child of evidence"
    );
    // The element forms read equal to the attribute form.
    let attribute = read_ok(
        "the attribute form",
        &case(&rel("rel", " source=\"report\" target=\"testing\"", ""), ""),
    );
    for (what, attributes, content) in [
        (
            "<source xmi:idref> and <target href>",
            "",
            "<source xmi:idref=\"report\"/><target href=\"#testing\"/>",
        ),
        (
            "<source href> and <target xmi:idref>",
            "",
            "<source href=\"#report\"/><target xmi:idref=\"testing\"/>",
        ),
        (
            "a source attribute and a <target> element",
            " source=\"report\"",
            "<target href=\"#testing\"/>",
        ),
    ] {
        let doc = read_ok(what, &case(&rel("rel", attributes, content), ""));
        assert_eq!(
            semantic_compare(&doc, &attribute),
            [],
            "{ROW}: {what}: does not read equal to the attribute form"
        );
        assert!(
            doc == attribute,
            "{ROW}: {what}: reads != to the attribute form"
        );
    }
    // Several target elements accumulate in order.
    let doc = read_ok(
        "two target elements",
        &case(
            &rel(
                "rel",
                " source=\"report\"",
                "<target xmi:idref=\"tester\"/><target href=\"#testing\"/>",
            ),
            "",
        ),
    );
    assert_eq!(
        ends_of(&doc, "rel"),
        (owned(&["report"]), owned(&["tester", "testing"])),
        "{ROW}: two target elements are not read in order"
    );
}

#[test]
fn sacm23_art_010_reads_the_base_features_utility_elements_and_properties_of_an_artifact_asset() {
    let what = "a relationship with every ArtifactAsset feature";
    let content = "<description xmi:id=\"d1\"><content><value content=\"D\"/></content></description>\
                   <note xmi:id=\"n1\"><content><value content=\"N\"/></content></note>\
                   <implementationConstraint xmi:id=\"ic1\"><content><value content=\"IC\"/></content>\
                   </implementationConstraint>\
                   <taggedValue xmi:id=\"tv1\"><key><value content=\"K\"/></key>\
                   <content><value content=\"V\"/></content></taggedValue>\
                   <property xmi:id=\"rp1\"><name content=\"RP1\"/></property>";
    let input = case(
        &format!(
            "{}{}",
            rel(
                "rel",
                " gid=\"REL-1\" isCitation=\"true\" citedElement=\"rel2\" isAbstract=\"true\" \
                 abstractForm=\"rel2\" source=\"report\" target=\"testing\"",
                content
            ),
            rel("rel2", " source=\"report\" target=\"tester\"", "")
        ),
        "",
    )
    .replacen(
        "<name content=\"rel\"/>",
        "<name lang=\"en\" content=\"Produced by\"/>",
        1,
    );
    let doc = read_ok(what, &input);
    let found = element(&doc, "rel");
    let ids = |elements: Vec<SacmElement<'_>>| -> Vec<String> {
        elements
            .iter()
            .map(|element| element.id().as_str().to_owned())
            .collect()
    };
    assert_eq!(
        (
            found.name().cloned(),
            found.gid(),
            found.is_citation(),
            found.cited_element().map(ElementId::as_str),
            found.is_abstract(),
            found.abstract_form().map(ElementId::as_str),
        ),
        (
            Some(LangString::new("Produced by").with_lang("en")),
            Some("REL-1"),
            true,
            Some("rel2"),
            true,
            Some("rel2"),
        ),
        "{ROW}: {what}: the name, gid, isCitation, citedElement, isAbstract and abstractForm"
    );
    assert_eq!(
        (
            ids(found.descriptions().collect()),
            ids(found.notes().collect()),
            ids(found.implementation_constraints().collect()),
            ids(found.tagged_values().collect()),
            ids(found.properties().collect()),
        ),
        (
            vec!["d1".to_owned()],
            vec!["n1".to_owned()],
            vec!["ic1".to_owned()],
            vec!["tv1".to_owned()],
            vec!["rp1".to_owned()],
        ),
        "{ROW}: {what}: the utility elements and the property"
    );
    assert_eq!(
        strs(found.sources()),
        ["report"],
        "{ROW}: {what}: sources()"
    );
    let text = round_trip(what, &doc, Mode::Strict);
    assert!(
        text.contains("<property xmi:id=\"rp1\">"),
        "{ROW}: {what}: the property is not written\n{text}"
    );
}

#[test]
fn sacm23_art_010_other_placements_and_the_features_of_other_classes_are_preserved() {
    // Untyped, under a role of an abstract type: MissingType (SACM-XMI-005).
    let (input, marks) = marked(&case(&format!("{MARK}<artifactElement xmi:id=\"x\"/>"), ""));
    let error = refused("an untyped artifactElement", &input);
    assert_eq!(
        (error.code(), error.location()),
        (Some(Code::XMI_MISSING_TYPE), marks.first().copied()),
        "{ROW}: an untyped artifactElement is not SACM-XMI-005 at its start tag: {error:?}"
    );
    // Under an ArgumentPackage's argumentElement, which admits no
    // ArtifactAsset: kept whole as preserved content of the package.
    let template = case("", "").replacen(
        "  </argumentPackage>",
        &format!(
            "    {MARK}<argumentElement xmi:type=\"sacm:ArtifactAssetRelationship\" xmi:id=\"x\" \
             source=\"report\" target=\"testing\"><name content=\"X\"/></argumentElement>\n  \
             </argumentPackage>"
        ),
        1,
    );
    let doc = assert_preserved_at("the kind in argumentElement", &template, "p");
    assert!(
        doc.get("x").is_none(),
        "{ROW}: the kind in argumentElement was read as an element"
    );
    // The features of an AssertedRelationship or an Assertion, which an
    // ArtifactAsset lacks: kept as preserved content of the relationship.
    for (what, attribute, content) in [
        (
            "isCounter=\"true\"",
            format!(" {MARK}isCounter=\"true\""),
            String::new(),
        ),
        (
            "reasoning=\"c1\"",
            format!(" {MARK}reasoning=\"c1\""),
            String::new(),
        ),
        (
            "metaClaim=\"c1\"",
            format!(" {MARK}metaClaim=\"c1\""),
            String::new(),
        ),
        (
            "assertionDeclaration=\"axiomatic\"",
            format!(" {MARK}assertionDeclaration=\"axiomatic\""),
            String::new(),
        ),
        (
            "an <isCounter> element",
            String::new(),
            format!("{MARK}<isCounter>true</isCounter>"),
        ),
        (
            "a <metaClaim> element",
            String::new(),
            format!("{MARK}<metaClaim xmi:idref=\"c1\"/>"),
        ),
        (
            "a <reasoning> element",
            String::new(),
            format!("{MARK}<reasoning xmi:idref=\"c1\"/>"),
        ),
    ] {
        let what = format!("{what} on a relationship");
        let doc = assert_preserved_at(
            &what,
            &case(
                &rel(
                    "rel",
                    &format!(" source=\"report\" target=\"testing\"{attribute}"),
                    &content,
                ),
                "",
            ),
            "rel",
        );
        assert_eq!(
            (
                element(&doc, "rel").kind(),
                strs(element(&doc, "rel").sources())
            ),
            (ElementKind::ArtifactAssetRelationship, vec!["report"]),
            "{ROW}: {what}: the relationship is not read beside the preserved content"
        );
    }
}

// =====================================================================================
// Item 2: repeats and empty ends
// =====================================================================================

#[test]
fn sacm23_art_010_an_attribute_and_elements_for_one_end_are_repeated_value() {
    for (end, attribute, element_form) in [
        (
            "source",
            " source=\"report\"",
            "<source xmi:idref=\"fta\"/>",
        ),
        (
            "target",
            " target=\"testing\"",
            "<target xmi:idref=\"tester\"/>",
        ),
    ] {
        let other = if end == "source" {
            " target=\"testing\""
        } else {
            " source=\"report\""
        };
        let what = format!("a {end} attribute and a {end} element");
        let template = case(
            &rel(
                "rel",
                &format!("{other} {MARK}{}", attribute.trim_start()),
                &format!("{MARK}{element_form}"),
            ),
            "",
        );
        let (input, marks) = marked(&template);
        let error = refused(&what, &input);
        assert!(
            matches!(
                &error,
                ReadError::RepeatedValue { element, property, .. }
                    if element.as_str() == "rel" && property == end
            ),
            "{ROW}: {what}: not RepeatedValue {{ rel, {end} }}: {error:?}"
        );
        assert_eq!(
            (error.code(), error.locations(), error.location()),
            (
                Some(Code::MULTIPLICITY_VIOLATION),
                marks.as_slice(),
                marks.get(1).copied()
            ),
            "{ROW}: {what}: not SACM-MULT-001 at both occurrences"
        );
        // Tolerant: the attribute is read, the element kept, with one
        // Warning at it (LIB-COMPAT-001).
        let output = match read(Mode::Tolerant, &input) {
            Ok(output) => output,
            Err(error) => panic!("{ROW}: {what}: a tolerant read refused it: {error:?}"),
        };
        assert_eq!(
            reported(output.diagnostics()),
            [(
                Code::MULTIPLICITY_VIOLATION,
                Severity::Warning,
                marks.get(1).copied(),
                list(&["rel"])
            )],
            "{ROW}: {what}: a tolerant read does not report one Warning at the element"
        );
        let doc = output.into_document();
        assert!(
            element(&doc, "rel").has_preserved_content(),
            "{ROW}: {what}: the element is not kept as preserved content"
        );
        let kept_id = if end == "source" { "fta" } else { "tester" };
        let text = write_in(&what, &doc, Mode::Tolerant);
        assert!(
            line_with(&text, "rel").contains(attribute)
                && text.contains(&format!("<{end} "))
                && text.contains(&format!("xmi:idref=\"{kept_id}\" />")),
            "{ROW}: {what}: a tolerant write does not write back the attribute and the kept \
             element\n{text}"
        );
    }
}

#[test]
fn sacm23_art_010_a_target_given_as_an_attribute_and_elements_is_not_called_single_valued() {
    // An ArtifactAssetRelationship's `target` is multi-valued (§12.14
    // `[1..*]`): an attribute and an element for it break rule 2h, as for
    // `source`, and the refusal says so, naming the element's kind.
    let template = case(
        &rel(
            "rel",
            &format!(" source=\"report\" {MARK}target=\"testing\""),
            &format!("{MARK}<target xmi:idref=\"tester\"/>"),
        ),
        "",
    );
    let (input, marks) = marked(&template);
    let error = refused("a target attribute and a target element", &input);
    assert!(
        matches!(
            &error,
            ReadError::RepeatedValue { element, kind: ElementKind::ArtifactAssetRelationship, property, .. }
                if element.as_str() == "rel" && property == "target"
        ),
        "{ROW}: not RepeatedValue {{ rel, ArtifactAssetRelationship, target }}: {error:?}"
    );
    assert_eq!(
        error.to_string(),
        format!(
            "the property \"target\" of the element \"rel\" is given both as an attribute and as \
             elements, which XMI 2.5.1 rule 2h forbids, at {}; {}",
            marks[0], marks[1]
        ),
        "{ROW}: the message of an ArtifactAssetRelationship's repeated target"
    );
    // An AssertedInference's `target` stays single-valued (§11.13 `[1]`).
    let template = case("", "").replacen(
        "target=\"c1\"><name content=\"I1\"/>",
        &format!("{MARK}target=\"c1\"><name content=\"I1\"/>{MARK}<target xmi:idref=\"c2\"/>"),
        1,
    );
    let (input, marks) = marked(&template);
    let error = refused("an inference's target attribute and element", &input);
    assert!(
        matches!(
            &error,
            ReadError::RepeatedValue { element, kind: ElementKind::AssertedInference, property, .. }
                if element.as_str() == "i1" && property == "target"
        ),
        "{ROW}: not RepeatedValue {{ i1, AssertedInference, target }}: {error:?}"
    );
    assert_eq!(
        error.to_string(),
        format!(
            "the single-valued property \"target\" of the element \"i1\" is given 2 times, at \
             {}; {}",
            marks[0], marks[1]
        ),
        "{ROW}: the message of an AssertedInference's repeated target"
    );
}

#[test]
fn sacm23_art_010_a_repeated_id_is_held_twice_and_is_one_val_022_warning() {
    let what = "source=\"report report\"";
    let doc = read_ok(
        what,
        &case(
            &rel("rel", " source=\"report report\" target=\"testing\"", ""),
            "",
        ),
    );
    assert_eq!(
        strs(element(&doc, "rel").sources()),
        ["report", "report"],
        "{ROW}: {what}: the repeat is not held twice"
    );
    assert_eq!(
        findings(&doc),
        [expected(
            Code::MULTIPLICITY_VIOLATION,
            Severity::Warning,
            "SACM23-VAL-022",
            &["rel", "report"]
        )],
        "{ROW}: {what}: not one SACM-MULT-001 Warning with clause 12.14 on [rel, report]"
    );
    let text = round_trip(what, &doc, Mode::Strict);
    assert!(
        line_with(&text, "rel").contains(" source=\"report report\""),
        "{ROW}: {what}: the repeat is not written back\n{text}"
    );
    let what = "target=\"testing tester testing\"";
    let doc = read_ok(
        what,
        &case(
            &rel(
                "rel",
                " source=\"report\" target=\"testing tester testing\"",
                "",
            ),
            "",
        ),
    );
    assert_eq!(
        findings(&doc),
        [expected(
            Code::MULTIPLICITY_VIOLATION,
            Severity::Warning,
            "SACM23-VAL-022",
            &["rel", "testing"]
        )],
        "{ROW}: {what}: not one SACM-MULT-001 Warning with clause 12.14 on [rel, testing]"
    );
    let text = round_trip(what, &doc, Mode::Strict);
    assert!(
        line_with(&text, "rel").ends_with(" target=\"testing tester testing\">"),
        "{ROW}: {what}: the repeat is not written back\n{text}"
    );
}

#[test]
fn sacm23_art_010_a_missing_end_reads_writes_back_and_is_one_mult_001_error_per_end() {
    let no_source = "ArtifactAssetRelationship \"rel\" has no source (source:ArtifactAsset[1..*])";
    let no_target = "ArtifactAssetRelationship \"rel\" has no target (target:ArtifactAsset[1..*])";
    for (what, attributes, messages) in [
        ("no source", " target=\"testing\"", vec![no_source]),
        ("no target", " source=\"report\"", vec![no_target]),
        ("neither end", "", vec![no_source, no_target]),
    ] {
        let doc = read_ok(what, &case(&rel("rel", attributes, ""), ""));
        let found = validation::validate(&doc);
        let mut got: Vec<(Finding, String)> = found
            .iter()
            .map(|found| (finding(found), found.message().to_owned()))
            .collect();
        got.sort_by(|a, b| a.1.cmp(&b.1));
        let want: Vec<(Finding, String)> = messages
            .iter()
            .map(|&message| {
                (
                    expected(Code::MULTIPLICITY_VIOLATION, Severity::Error, ROW, &["rel"]),
                    message.to_owned(),
                )
            })
            .collect();
        assert_eq!(
            got, want,
            "{ROW}: {what}: not one SACM-MULT-001 Error per missing end"
        );
        for mode in MODES {
            let text = round_trip(what, &doc, mode);
            assert_eq!(
                line_with(&text, "rel"),
                format!("{REL_TAG}{attributes}>"),
                "{ROW}: {what}: not written back with nothing added in {mode:?} mode\n{text}"
            );
        }
    }
}

// =====================================================================================
// Item 3: dangling and mistyped ends
// =====================================================================================

#[test]
fn sacm23_art_010_a_dangling_end_is_ref_001_and_a_strict_write_refuses_it() {
    for (what, attributes, ghost) in [
        (
            "source=\"report ghost\"",
            " source=\"report ghost\" target=\"testing\"",
            "ghost",
        ),
        (
            "target=\"lost\"",
            " source=\"report\" target=\"lost\"",
            "lost",
        ),
    ] {
        let doc = read_ok(what, &case(&rel("rel", attributes, ""), ""));
        assert_eq!(
            findings(&doc),
            [expected(
                Code::REF_DANGLING,
                Severity::Error,
                "SACM23-VAL-003",
                &["rel", ghost]
            )],
            "{ROW}: {what}: not one SACM-REF-001 Error with clause 12.14"
        );
        let error = match xmi::write(&doc, &WriteOptions::default().with_mode(Mode::Strict)) {
            Ok(text) => panic!("{ROW}: {what}: a strict write accepted it:\n{text}"),
            Err(error) => error,
        };
        assert!(
            matches!(error, WriteError::Invalid { .. })
                && error
                    .diagnostics()
                    .iter()
                    .all(|found| found.code() == Code::REF_DANGLING),
            "{ROW}: {what}: a strict write is not Invalid with SACM-REF-001: {error:?}"
        );
        let text = round_trip(what, &doc, Mode::Tolerant);
        assert!(
            line_with(&text, "rel").contains(&format!("{ghost}\"")),
            "{ROW}: {what}: the dangling id is not written back\n{text}"
        );
    }
}

#[test]
fn sacm23_art_010_an_end_naming_no_artifact_asset_is_one_ref_002_and_both_saves_write_it() {
    // A Claim, an ArtifactReference, an ArtifactGroup and an ArtifactPackage
    // are ArtifactElements but no ArtifactAssets (§12.6, §12.3, §12.2).
    for wrong in ["c1", "ar1", "grp", "other"] {
        for (end, attributes) in [
            (
                "source",
                format!(" source=\"report {wrong}\" target=\"testing\""),
            ),
            (
                "target",
                format!(" source=\"report\" target=\"{wrong} testing\""),
            ),
        ] {
            let what = format!("{wrong} as a {end}");
            let doc = read_ok(&what, &case(&rel("rel", &attributes, ""), ""));
            assert_eq!(
                findings(&doc),
                [expected(
                    Code::REF_WRONG_TYPE,
                    Severity::Error,
                    "SACM23-VAL-004",
                    &["rel", wrong]
                )],
                "{ROW}: {what}: not one SACM-REF-002 Error with clause 12.14"
            );
            let text = write_in(&what, &doc, Mode::Tolerant);
            assert_eq!(
                write_in(&what, &doc, Mode::Strict),
                text,
                "{ROW}: {what}: the strict write is not the tolerant one"
            );
            assert!(
                line_with(&text, "rel").contains(&attributes),
                "{ROW}: {what}: the end is not written back\n{text}"
            );
        }
    }
}

#[test]
fn sacm23_art_010_an_end_naming_any_artifact_asset_gives_no_finding() {
    // Every ArtifactAsset kind (§12.6): Artifact, Activity, Event, Resource,
    // Technique, Participant, Property and ArtifactAssetRelationship, the
    // relationship itself included.
    let all = "report testing released pdf fta tester pr1 rel rel2";
    let what = "every ArtifactAsset kind in both ends";
    let doc = read_ok(
        what,
        &case(
            &format!(
                "{}{}",
                rel("rel", &format!(" source=\"{all}\" target=\"{all}\""), ""),
                rel("rel2", " source=\"rel\" target=\"rel\"", "")
            ),
            "",
        ),
    );
    assert_eq!(
        findings(&doc),
        [],
        "{ROW}: {what}: validate reports a finding"
    );
    round_trip(what, &doc, Mode::Strict);
}

// =====================================================================================
// Item 4: writing, the fixture and the golden file
// =====================================================================================

#[test]
fn sacm23_art_010_is_written_in_attribute_order_with_repeats_and_no_empty_end() {
    let what = "every feature, given in another order";
    let doc = read_ok(
        what,
        &case(
            &format!(
                "{}{}",
                rel(
                    "rel",
                    " target=\"testing tester\" source=\"report report\" abstractForm=\"rel2\" \
                     citedElement=\"report\" isAbstract=\"true\" isCitation=\"true\" gid=\"g-rel\"",
                    ""
                ),
                rel("rel2", " source=\"report\" target=\"testing\"", "")
            ),
            "",
        ),
    );
    let order = format!(
        "{REL_TAG} gid=\"g-rel\" isCitation=\"true\" isAbstract=\"true\" citedElement=\"report\" \
         abstractForm=\"rel2\" source=\"report report\" target=\"testing tester\">"
    );
    for mode in MODES {
        let text = round_trip(what, &doc, mode);
        assert_eq!(
            line_with(&text, "rel"),
            order,
            "{ROW}: {what}: not written in the order ... abstractForm source target in {mode:?} \
             mode\n{text}"
        );
    }
    let what = "both ends as elements";
    let doc = read_ok(
        what,
        &case(
            &rel(
                "rel",
                "",
                "<source xmi:idref=\"report\"/><source href=\"#fta\"/>\
                 <target xmi:idref=\"tester\"/><target xmi:idref=\"tester\"/>",
            ),
            "",
        ),
    );
    let text = round_trip(what, &doc, Mode::Strict);
    assert_eq!(
        line_with(&text, "rel"),
        format!("{REL_TAG} source=\"report fta\" target=\"tester tester\">"),
        "{ROW}: {what}: not written as two attributes with the repeat\n{text}"
    );
}

fn fixture_text() -> String {
    std::fs::read_to_string(FIXTURE)
        .unwrap_or_else(|error| panic!("{ROW}: cannot read {FIXTURE}: {error}"))
}

fn golden_text() -> String {
    std::fs::read_to_string(GOLDEN)
        .unwrap_or_else(|error| panic!("{ROW}: cannot read {GOLDEN}: {error}"))
}

#[test]
fn sacm23_art_010_the_fixture_reads_validates_and_writes_exactly_the_golden_file() {
    let doc = read_ok("the fixture", &fixture_text());
    assert_eq!(
        (
            children(&doc, "evidence"),
            strs(element(&doc, "produced_by").sources()),
            strs(element(&doc, "derived").sources()),
            children(&doc, "produced_by"),
            element(&doc, "derived").gid(),
        ),
        (
            vec![
                "report".to_owned(),
                "testing".to_owned(),
                "tester".to_owned(),
                "produced_by".to_owned(),
                "derived".to_owned()
            ],
            vec!["report"],
            vec!["produced_by"],
            vec!["produced_by_reviewed".to_owned()],
            Some("REL-2"),
        ),
        "{ROW}: the fixture's children, sources, property and gid"
    );
    assert_eq!(
        validation::validate(&doc),
        [],
        "{ROW}: the fixture does not validate cleanly"
    );
    let golden = golden_text();
    for mode in MODES {
        assert_eq!(
            write_in("the fixture", &doc, mode),
            golden,
            "{ROW}: the fixture does not write the golden file's bytes in {mode:?} mode"
        );
    }
    let back = read_ok("the golden file", &golden);
    assert_eq!(
        semantic_compare(&doc, &back),
        [],
        "{ROW}: the golden file does not read back equal to the fixture"
    );
    assert!(
        back == doc,
        "{ROW}: the golden file does not read back == to the fixture"
    );
    for mode in MODES {
        assert_eq!(
            write_in("the golden file", &back, mode),
            golden,
            "{ROW}: writing the golden file again gives other bytes in {mode:?} mode"
        );
    }
}

#[test]
fn sacm23_art_010_the_fixture_holds_the_targets_of_both_relationships() {
    let doc = read_ok("the fixture", &fixture_text());
    assert_eq!(
        (ends_of(&doc, "produced_by"), ends_of(&doc, "derived")),
        (
            (owned(&["report"]), owned(&["testing", "tester"])),
            (owned(&["produced_by"]), owned(&["report"]))
        ),
        "{ROW}: the fixture's sources and targets"
    );
}

// =====================================================================================
// Item 5: the ends and the accessors
// =====================================================================================

#[test]
fn sacm23_art_010_artifact_source_and_target_are_named_owned_and_bounded_for_the_relationship_only()
{
    for (end, name) in [
        (ReferenceEnd::ArtifactSource, "source"),
        (ReferenceEnd::ArtifactTarget, "target"),
    ] {
        assert_eq!(end.name(), name, "{ROW}: the name of {end:?}");
        for &kind in ElementKind::ALL {
            let relationship = kind == ElementKind::ArtifactAssetRelationship;
            assert_eq!(
                (end.applies_to(kind), end.bounds(kind)),
                (relationship, relationship.then_some((1, None))),
                "{ROW}: {end:?} on a {kind}: applies_to and bounds"
            );
        }
    }
    // The AssertedRelationship's ends do not apply to the relationship.
    for end in [
        ReferenceEnd::Source,
        ReferenceEnd::Target,
        ReferenceEnd::Reasoning,
        ReferenceEnd::MetaClaim,
    ] {
        assert_eq!(
            (
                end.applies_to(ElementKind::ArtifactAssetRelationship),
                end.bounds(ElementKind::ArtifactAssetRelationship)
            ),
            (false, None),
            "{ROW}: {end:?} applies to an ArtifactAssetRelationship"
        );
    }
}

#[test]
fn sacm23_art_010_reference_end_all_lists_these_two_directly_after_artifact_group_member() {
    // The ends of slice 0057 follow ArtifactGroupMember, and the Terminology
    // ends of slice 0062 follow them (ADR 0034 amendments 2 and 3).
    assert_eq!(
        ReferenceEnd::ALL,
        [
            ReferenceEnd::CitedElement,
            ReferenceEnd::AbstractForm,
            ReferenceEnd::MetaClaim,
            ReferenceEnd::Reasoning,
            ReferenceEnd::Source,
            ReferenceEnd::Target,
            ReferenceEnd::Structure,
            ReferenceEnd::ReferencedArtifactElement,
            ReferenceEnd::GroupMember,
            ReferenceEnd::ArtifactGroupMember,
            ReferenceEnd::ArtifactSource,
            ReferenceEnd::ArtifactTarget,
            ReferenceEnd::Category,
            ReferenceEnd::Element,
            ReferenceEnd::Origin,
            ReferenceEnd::TerminologyGroupMember,
        ],
        "{ROW}: ReferenceEnd::ALL"
    );
    let after = ReferenceEnd::ALL
        .iter()
        .position(|&end| end == ReferenceEnd::ArtifactGroupMember)
        .map(|index| &ReferenceEnd::ALL[index + 1..index + 3]);
    assert_eq!(
        after,
        Some([ReferenceEnd::ArtifactSource, ReferenceEnd::ArtifactTarget].as_slice()),
        "{ROW}: ArtifactSource and ArtifactTarget do not follow ArtifactGroupMember"
    );
}

#[test]
fn sacm23_art_010_no_kind_has_two_ends_of_the_same_name() {
    // The reader resolves an end by the element's kind (ADR 0034 amendment
    // 2), so no kind may hold two ends of one XMI name.
    for &kind in ElementKind::ALL {
        let names: Vec<&str> = ReferenceEnd::ALL
            .iter()
            .filter(|end| end.applies_to(kind))
            .map(|end| end.name())
            .collect();
        let distinct: BTreeSet<&str> = names.iter().copied().collect();
        assert_eq!(
            distinct.len(),
            names.len(),
            "{ROW}: a {kind} has two ends of one name: {names:?}"
        );
    }
    // The relationship has the two SACMElement ends and its own two.
    let names: Vec<&str> = ReferenceEnd::ALL
        .iter()
        .filter(|end| end.applies_to(ElementKind::ArtifactAssetRelationship))
        .map(|end| end.name())
        .collect();
    assert_eq!(
        names,
        ["citedElement", "abstractForm", "source", "target"],
        "{ROW}: the ends of an ArtifactAssetRelationship"
    );
}

#[test]
fn sacm23_art_010_sources_and_targets_answer_for_both_relationship_kinds() {
    let doc = read_ok("a relationship and an inference", &with_rel(""));
    let found = element(&doc, "rel");
    assert_eq!(
        (
            found.sources(),
            found.targets(),
            found.target(),
            found.references(ReferenceEnd::Source),
            found.references(ReferenceEnd::Target),
        ),
        (
            found.references(ReferenceEnd::ArtifactSource),
            found.references(ReferenceEnd::ArtifactTarget),
            None,
            [].as_slice(),
            [].as_slice(),
        ),
        "{ROW}: the accessors of the relationship"
    );
    assert_eq!(
        (strs(found.sources()), strs(found.targets())),
        (vec!["report"], vec!["testing", "tester"]),
        "{ROW}: sources() and targets() of the relationship"
    );
    let inference = element(&doc, "i1");
    assert_eq!(
        (
            inference.sources(),
            inference.targets(),
            inference.references(ReferenceEnd::ArtifactSource),
            inference.references(ReferenceEnd::ArtifactTarget),
        ),
        (
            inference.references(ReferenceEnd::Source),
            inference.references(ReferenceEnd::Target),
            [].as_slice(),
            [].as_slice(),
        ),
        "{ROW}: the accessors of an AssertedInference"
    );
    assert_eq!(
        (
            strs(inference.targets()),
            inference.target().map(ElementId::as_str)
        ),
        (vec!["c1"], Some("c1")),
        "{ROW}: targets() and target() of an AssertedInference"
    );
    for other in ["c1", "report", "evidence"] {
        let found = element(&doc, other);
        assert!(
            found.sources().is_empty() && found.targets().is_empty(),
            "{ROW}: {other} is no relationship but has sources or targets"
        );
    }
}

// =====================================================================================
// Item 6: comparison
// =====================================================================================

#[test]
fn sacm23_art_010_semantic_compare_reports_artifact_sources_and_artifact_targets() {
    let with = |sources: &str, targets: &str| {
        read_ok(
            "a relationship",
            &case(
                &rel(
                    "rel",
                    &format!(" source=\"{sources}\" target=\"{targets}\""),
                    "",
                ),
                "",
            ),
        )
    };
    let split = |ids: &str| list(&ids.split(' ').collect::<Vec<_>>());
    // Order and repeats count, as for every multi-valued end.
    for (left, right) in [
        ("report fta", "fta report"),
        ("report", "report report"),
        ("report", "fta"),
    ] {
        let found = semantic_compare(&with(left, "testing"), &with(right, "testing"));
        assert_eq!(
            found,
            [Difference::ArtifactSources {
                id: ElementId::from("rel"),
                left: split(left),
                right: split(right),
            }],
            "{ROW}: sources {left:?} vs {right:?}: not one ArtifactSources difference"
        );
        assert_eq!(
            found[0].element_id(),
            Some(&ElementId::from("rel")),
            "{ROW}: element_id() does not name the relationship"
        );
    }
    for (left, right) in [
        ("testing tester", "tester testing"),
        ("testing", "testing testing"),
        ("testing", "testing tester"),
    ] {
        let found = semantic_compare(&with("report", left), &with("report", right));
        assert_eq!(
            found,
            [Difference::ArtifactTargets {
                id: ElementId::from("rel"),
                left: split(left),
                right: split(right),
            }],
            "{ROW}: targets {left:?} vs {right:?}: not one ArtifactTargets difference"
        );
    }
    // In feature order: gid, abstractForm, then the two ends, then the name.
    let left = read_ok(
        "the left",
        &case(
            &rel(
                "rel",
                " gid=\"L\" abstractForm=\"report\" source=\"report\" target=\"testing\"",
                "",
            ),
            "",
        ),
    );
    let right = read_ok(
        "the right",
        &case(&rel("rel", " source=\"fta\" target=\"tester\"", ""), "").replacen(
            "<name content=\"rel\"/>",
            "<name content=\"Other\"/>",
            1,
        ),
    );
    let found = semantic_compare(&left, &right);
    assert!(
        matches!(
            found.as_slice(),
            [
                Difference::Gid { .. },
                Difference::AbstractForm { .. },
                Difference::ArtifactSources { .. },
                Difference::ArtifactTargets { .. },
                Difference::Name { .. },
            ]
        ),
        "{ROW}: not Gid, AbstractForm, ArtifactSources, ArtifactTargets, Name: {found:?}"
    );
    assert!(
        found.iter().all(|difference| !matches!(
            difference,
            Difference::Sources { .. } | Difference::Target { .. }
        )),
        "{ROW}: a relationship is compared by an AssertedRelationship's ends: {found:?}"
    );
}

#[test]
fn sacm23_art_010_the_differences_display_their_end_and_both_lists() {
    let targets = Difference::ArtifactTargets {
        id: ElementId::from("rel"),
        left: list(&["testing"]),
        right: list(&["testing", "tester"]),
    };
    assert_eq!(
        targets.to_string(),
        "element \"rel\" differs in target: [\"testing\"] on the left, [\"testing\", \"tester\"] \
         on the right",
        "{ROW}: the Display of ArtifactTargets"
    );
    let sources = Difference::ArtifactSources {
        id: ElementId::from("rel"),
        left: list(&["report", "report"]),
        right: Vec::new(),
    };
    assert_eq!(
        sources.to_string(),
        "element \"rel\" differs in source: [\"report\", \"report\"] on the left, [] on the right",
        "{ROW}: the Display of ArtifactSources"
    );
}

// =====================================================================================
// Item 7: CreateArtifactAssetRelationship
// =====================================================================================

/// The relationship create `rel_<n>` under `parent`, from `sources` to
/// `targets`, with the caller id `id` if given.
fn create_rel(
    parent: &str,
    id: Option<&str>,
    sources: &[&str],
    targets: &[&str],
) -> CreateArtifactAssetRelationship {
    let create = CreateArtifactAssetRelationship::new(
        parent,
        named("N"),
        sources.iter().copied(),
        targets.iter().copied(),
    );
    match id {
        Some(id) => create.with_id(id),
        None => create,
    }
}

fn assert_send_sync<T: Send + Sync + Clone + std::fmt::Debug + PartialEq + Eq>() {}

#[test]
fn sacm23_art_010_create_reports_what_it_holds_and_converts_into_its_operation() {
    assert_send_sync::<CreateArtifactAssetRelationship>();
    let create = CreateArtifactAssetRelationship::new(
        "evidence",
        named("Produced by"),
        ["report", "report"],
        ["testing", "tester"],
    );
    assert_eq!(
        (
            create.parent().as_str(),
            create.id(),
            create.name(),
            strs(create.sources()),
            strs(create.targets())
        ),
        (
            "evidence",
            None,
            &named("Produced by"),
            vec!["report", "report"],
            vec!["testing", "tester"]
        ),
        "{ROW}: what new() holds"
    );
    let with_id = create.clone().with_id("rel9");
    assert_eq!(
        with_id.id().map(ElementId::as_str),
        Some("rel9"),
        "{ROW}: with_id"
    );
    let operation = Operation::from(with_id.clone());
    assert!(
        matches!(&operation, Operation::CreateArtifactAssetRelationship(held) if *held == with_id),
        "{ROW}: not converted into its Operation variant: {operation:?}"
    );
    assert_eq!(
        operation.operation_name(),
        "CreateArtifactAssetRelationship",
        "{ROW}: operation_name()"
    );
    // Applied: the ids in order, repeats kept.
    let mut doc = read_ok("the case", &case("", ""));
    let mutation = applied(&mut doc, with_id);
    assert_eq!(
        (
            strs(mutation.created_ids()),
            mutation.changed_ids(),
            mutation.deleted_ids()
        ),
        (vec!["rel9"], [].as_slice(), [].as_slice()),
        "{ROW}: the mutation of a create"
    );
    let found = element(&doc, "rel9");
    assert_eq!(
        (
            found.kind(),
            found.parent().map(|parent| parent.id().as_str().to_owned()),
            found.name().cloned(),
            ends_of(&doc, "rel9")
        ),
        (
            ElementKind::ArtifactAssetRelationship,
            Some("evidence".to_owned()),
            Some(named("Produced by")),
            (owned(&["report", "report"]), owned(&["testing", "tester"]))
        ),
        "{ROW}: the created relationship"
    );
    let text = write_in("the created relationship", &doc, Mode::Strict);
    assert!(
        line_with(&text, "rel9").starts_with(
            "<artifactElement xmi:type=\"sacm:ArtifactAssetRelationship\" xmi:id=\"rel9\" \
             source=\"report report\" target=\"testing tester\">"
        ),
        "{ROW}: the created relationship is not written in artifactElement\n{text}"
    );
}

#[test]
fn sacm23_art_010_create_legal_parents_refusals_and_generated_id() {
    let mut doc = read_ok("the case", &case("", ""));
    let first = applied(
        &mut doc,
        create_rel("evidence", None, &["report"], &["testing"]),
    );
    let second = applied(
        &mut doc,
        create_rel("other", None, &["report"], &["testing"]),
    );
    assert_eq!(
        (strs(first.created_ids()), strs(second.created_ids())),
        (
            vec!["artifact_asset_relationship_1"],
            vec!["artifact_asset_relationship_2"]
        ),
        "{ROW}: the generated ids, under both ArtifactPackages"
    );
    // Every other kind of parent is InvalidParent.
    for (parent, parent_kind) in [
        ("case", ElementKind::AssuranceCasePackage),
        ("p", ElementKind::ArgumentPackage),
        ("report", ElementKind::Artifact),
        ("grp", ElementKind::ArtifactGroup),
        (
            "artifact_asset_relationship_1",
            ElementKind::ArtifactAssetRelationship,
        ),
    ] {
        let error = refuse(
            &mut doc,
            create_rel(parent, None, &["report"], &["testing"]),
        );
        assert!(
            matches!(
                &error,
                EditError::InvalidParent { parent: p, parent_kind: k, kind, .. }
                    if p.as_str() == parent && *k == parent_kind
                        && *kind == ElementKind::ArtifactAssetRelationship
            ),
            "{ROW}: a relationship under the {parent_kind} {parent}: {error:?}"
        );
        assert_eq!(
            error.code(),
            Code::CMD_INVALID_PARENT,
            "{ROW}: not SACM-CMD-005"
        );
    }
    // A refusal moves no generated id.
    let next = applied(
        &mut doc,
        create_rel("evidence", None, &["report"], &["testing"]),
    );
    assert_eq!(
        strs(next.created_ids()),
        ["artifact_asset_relationship_3"],
        "{ROW}: a refusal moved the next generated id"
    );
    assert_eq!(
        validation::validate(&doc),
        [],
        "{ROW}: the created relationships do not validate cleanly"
    );
}

/// One case of
/// [`sacm23_art_010_create_checks_the_ends_in_order_after_the_parent_and_id`]:
/// what, parent, id, sources, targets, and the expected refusal (variant,
/// end, id).
type EndCase<'a> = (
    &'a str,
    &'a str,
    Option<&'a str>,
    &'a [&'a str],
    &'a [&'a str],
    (&'a str, &'a str, &'a str),
);

#[test]
#[expect(
    clippy::too_many_lines,
    reason = "one flat table of the refusals in order is easier to review than several"
)]
fn sacm23_art_010_create_checks_the_ends_in_order_after_the_parent_and_id() {
    let mut doc = read_ok("the case", &case("", ""));
    let variant = |error: &EditError| {
        format!("{error:?}")
            .split([' ', '{', '('])
            .next()
            .unwrap_or_default()
            .to_owned()
    };
    // (what, parent, id, sources, targets, the expected refusal: variant,
    // end, id).
    let cases: [EndCase<'_>; 16] = [
        (
            "a missing parent first",
            "nowhere",
            Some("1x"),
            &[],
            &[],
            ("ParentNotFound", "", "nowhere"),
        ),
        (
            "an illegal parent before the id",
            "p",
            Some("1x"),
            &[],
            &[],
            ("InvalidParent", "", "p"),
        ),
        (
            "an invalid id before the ends",
            "evidence",
            Some("1x"),
            &[],
            &[],
            ("InvalidId", "", "1x"),
        ),
        (
            "a duplicate id before the ends",
            "evidence",
            Some("report"),
            &[],
            &[],
            ("DuplicateId", "", "report"),
        ),
        (
            "no source, then no target",
            "evidence",
            None,
            &[],
            &[],
            ("TooFewReferences", "source", ""),
        ),
        (
            "no target",
            "evidence",
            None,
            &["report"],
            &[],
            ("TooFewReferences", "target", ""),
        ),
        (
            "no target before an unknown source",
            "evidence",
            None,
            &["ghost"],
            &[],
            ("TooFewReferences", "target", ""),
        ),
        (
            "an invalid source first",
            "evidence",
            None,
            &["a b", "x y"],
            &["c d"],
            ("InvalidReference", "source", "a b"),
        ),
        (
            "an invalid target before an unknown source",
            "evidence",
            None,
            &["ghost"],
            &["a b"],
            ("InvalidReference", "target", "a b"),
        ),
        (
            "an invalid target before a mistyped source",
            "evidence",
            None,
            &["c1"],
            &["a b"],
            ("InvalidReference", "target", "a b"),
        ),
        (
            "an unknown source first",
            "evidence",
            None,
            &["report", "ghost1", "ghost2"],
            &["ghost3"],
            ("ReferenceNotFound", "source", "ghost1"),
        ),
        (
            "an unknown target before a mistyped source",
            "evidence",
            None,
            &["c1"],
            &["ghost"],
            ("ReferenceNotFound", "target", "ghost"),
        ),
        (
            "a mistyped source first",
            "evidence",
            None,
            &["report", "c1", "grp"],
            &["ar1"],
            ("WrongReferenceType", "source", "c1"),
        ),
        (
            "a mistyped target",
            "evidence",
            None,
            &["report"],
            &["testing", "grp", "c1"],
            ("WrongReferenceType", "target", "grp"),
        ),
        (
            "a package as a target",
            "evidence",
            None,
            &["report"],
            &["other"],
            ("WrongReferenceType", "target", "other"),
        ),
        (
            "an ArtifactReference as a source",
            "evidence",
            None,
            &["ar1"],
            &["testing"],
            ("WrongReferenceType", "source", "ar1"),
        ),
    ];
    for (what, parent, id, sources, targets, (expected_variant, expected_end, expected_id)) in cases
    {
        let error = refuse(&mut doc, create_rel(parent, id, sources, targets));
        assert_eq!(
            variant(&error),
            expected_variant,
            "{ROW}: {what}: the refusal {error:?}"
        );
        let named_end_and_id = match &error {
            EditError::ParentNotFound { parent, .. } | EditError::InvalidParent { parent, .. } => {
                (String::new(), parent.as_str().to_owned())
            }
            EditError::InvalidId { id, .. } | EditError::DuplicateId { id, .. } => {
                (String::new(), id.as_str().to_owned())
            }
            EditError::TooFewReferences {
                end,
                given,
                required,
                ..
            } => {
                assert_eq!(
                    (*given, *required),
                    (0, 1),
                    "{ROW}: {what}: given and required"
                );
                ((*end).to_owned(), String::new())
            }
            EditError::InvalidReference { end, id, .. }
            | EditError::ReferenceNotFound { end, id, .. } => {
                ((*end).to_owned(), id.as_str().to_owned())
            }
            EditError::WrongReferenceType {
                end,
                id,
                kind,
                expected,
                ..
            } => {
                assert_eq!(
                    (*expected, element(&doc, id.as_str()).kind()),
                    ("ArtifactAsset", *kind),
                    "{ROW}: {what}: the expected class and the kind"
                );
                ((*end).to_owned(), id.as_str().to_owned())
            }
            other => panic!("{ROW}: {what}: an unexpected refusal {other:?}"),
        };
        assert_eq!(
            named_end_and_id,
            (expected_end.to_owned(), expected_id.to_owned()),
            "{ROW}: {what}: the end and id of {error:?}"
        );
    }
    // A refusal leaves the next generated id as it was.
    let created = applied(
        &mut doc,
        create_rel("evidence", None, &["report"], &["testing"]),
    );
    assert_eq!(
        strs(created.created_ids()),
        ["artifact_asset_relationship_1"],
        "{ROW}: a refusal moved the next generated id"
    );
}

// =====================================================================================
// Item 8: the reference edits
// =====================================================================================

#[test]
fn sacm23_art_010_reference_edits_keep_each_end_at_least_one_id() {
    let start = read_ok("a relationship", &with_rel(""));
    let too_few = |error: &EditError, end: &str| {
        matches!(
            error,
            EditError::TooFewReferences { end: e, given: 0, required: 1, .. } if *e == end
        ) && error.code() == Code::MULTIPLICITY_VIOLATION
    };
    for (end, name) in [
        (ReferenceEnd::ArtifactSource, "source"),
        (ReferenceEnd::ArtifactTarget, "target"),
    ] {
        let mut doc = start.clone();
        let error = refuse(&mut doc, SetReferences::clear("rel", end));
        assert!(
            too_few(&error, name),
            "{ROW}: clearing the {name}: {error:?}"
        );
        let error = refuse(
            &mut doc,
            SetReferences::new("rel", end, Vec::<ElementId>::new()),
        );
        assert!(too_few(&error, name), "{ROW}: setting no {name}: {error:?}");
        // A wrong type, through an edit.
        let error = refuse(&mut doc, AddReference::new("rel", end, "c1"));
        assert!(
            matches!(
                &error,
                EditError::WrongReferenceType { end: e, id, kind: ElementKind::Claim, expected: "ArtifactAsset", .. }
                    if *e == name && id.as_str() == "c1"
            ),
            "{ROW}: adding the Claim c1 as a {name}: {error:?}"
        );
    }
    // Set, add and remove.
    let mut doc = start.clone();
    applied(
        &mut doc,
        SetReferences::new("rel", ReferenceEnd::ArtifactSource, ["fta", "report"]),
    );
    applied(
        &mut doc,
        AddReference::new("rel", ReferenceEnd::ArtifactTarget, "report"),
    );
    applied(
        &mut doc,
        AddReference::new("rel", ReferenceEnd::ArtifactSource, "fta"),
    );
    assert_eq!(
        ends_of(&doc, "rel"),
        (
            owned(&["fta", "report", "fta"]),
            owned(&["testing", "tester", "report"])
        ),
        "{ROW}: after SetReferences and two AddReferences"
    );
    applied(
        &mut doc,
        RemoveReference::new("rel", ReferenceEnd::ArtifactTarget, "testing"),
    );
    applied(
        &mut doc,
        RemoveReference::new("rel", ReferenceEnd::ArtifactSource, "report"),
    );
    assert_eq!(
        ends_of(&doc, "rel"),
        (owned(&["fta", "fta"]), owned(&["tester", "report"])),
        "{ROW}: after two RemoveReferences"
    );
    // Removing the last id of either end.
    let mut doc = start;
    applied(
        &mut doc,
        RemoveReference::new("rel", ReferenceEnd::ArtifactTarget, "testing"),
    );
    let error = refuse(
        &mut doc,
        RemoveReference::new("rel", ReferenceEnd::ArtifactTarget, "tester"),
    );
    assert!(
        too_few(&error, "target"),
        "{ROW}: removing the last target: {error:?}"
    );
    let error = refuse(
        &mut doc,
        RemoveReference::new("rel", ReferenceEnd::ArtifactSource, "report"),
    );
    assert!(
        too_few(&error, "source"),
        "{ROW}: removing the last source: {error:?}"
    );
}

#[test]
fn sacm23_art_010_the_other_class_s_ends_are_feature_not_applicable_naming_that_class() {
    let start = read_ok("a relationship and an inference", &with_rel(""));
    for (element_id, kind, end, feature, display) in [
        (
            "rel",
            ElementKind::ArtifactAssetRelationship,
            ReferenceEnd::Source,
            "source",
            "the ArtifactAssetRelationship \"rel\" has no source of an AssertedRelationship",
        ),
        (
            "rel",
            ElementKind::ArtifactAssetRelationship,
            ReferenceEnd::Target,
            "target",
            "the ArtifactAssetRelationship \"rel\" has no target of an AssertedRelationship",
        ),
        (
            "rel",
            ElementKind::ArtifactAssetRelationship,
            ReferenceEnd::Reasoning,
            "reasoning",
            "the ArtifactAssetRelationship \"rel\" has no reasoning",
        ),
        (
            "i1",
            ElementKind::AssertedInference,
            ReferenceEnd::ArtifactSource,
            "source",
            "the AssertedInference \"i1\" has no source of an ArtifactAssetRelationship",
        ),
        (
            "i1",
            ElementKind::AssertedInference,
            ReferenceEnd::ArtifactTarget,
            "target",
            "the AssertedInference \"i1\" has no target of an ArtifactAssetRelationship",
        ),
        (
            "c1",
            ElementKind::Claim,
            ReferenceEnd::Source,
            "source",
            "the Claim \"c1\" has no source",
        ),
        (
            "c1",
            ElementKind::Claim,
            ReferenceEnd::ArtifactTarget,
            "target",
            "the Claim \"c1\" has no target",
        ),
    ] {
        let what = format!("{end:?} on {element_id}");
        for operation in [
            Operation::from(SetReferences::new(element_id, end, ["report"])),
            Operation::from(AddReference::new(element_id, end, "report")),
            Operation::from(RemoveReference::new(element_id, end, "report")),
        ] {
            let mut doc = start.clone();
            let error = refuse(&mut doc, operation);
            assert!(
                matches!(
                    &error,
                    EditError::FeatureNotApplicable { element, kind: k, feature: f, .. }
                        if element.as_str() == element_id && *k == kind && *f == feature
                ),
                "{ROW}: {what}: not FeatureNotApplicable {{ {element_id}, {kind}, {feature} }}: \
                 {error:?}"
            );
            assert_eq!(
                (error.code(), error.to_string()),
                (Code::CMD_FEATURE_NOT_APPLICABLE, display.to_owned()),
                "{ROW}: {what}: the code and the Display"
            );
        }
    }
}

// =====================================================================================
// Item 9: the delete policies
// =====================================================================================

#[test]
fn sacm23_art_010_deleting_a_source_or_target_is_referenced_or_cross_package_reference() {
    let reject = |input: &str, delete: &str| {
        let mut doc = read_ok("the case", input);
        refuse(
            &mut doc,
            DeleteElement::new(delete).with_package_policy(PackageDeletePolicy::DeleteRecursively),
        )
    };
    for (delete, end) in [("report", "source"), ("tester", "target")] {
        let error = reject(&with_rel(""), delete);
        assert!(
            matches!(
                &error,
                EditError::Referenced { holder, end: e, id, .. }
                    if holder.as_str() == "rel" && *e == end && id.as_str() == delete
            ),
            "{ROW}: deleting {delete}: not Referenced {{ rel, {end}, {delete} }}: {error:?}"
        );
        assert_eq!(
            error.code(),
            Code::CMD_DELETE_REFERENCED,
            "{ROW}: deleting {delete}: not SACM-CMD-002"
        );
        // From the package other: CrossPackageReference.
        let across = case("", &rel("rel2", " source=\"report\" target=\"tester\"", ""));
        let error = reject(&across, delete);
        assert!(
            matches!(
                &error,
                EditError::CrossPackageReference { holder, end: e, id, .. }
                    if holder.as_str() == "rel2" && *e == end && id.as_str() == delete
            ),
            "{ROW}: deleting {delete} across packages: not CrossPackageReference {{ rel2, {end}, \
             {delete} }}: {error:?}"
        );
        assert_eq!(
            error.code(),
            Code::CMD_EXTERNAL_REFERENCES,
            "{ROW}: deleting {delete} across packages: not SACM-CMD-007"
        );
    }
}

/// [`case`] with four relationships in `evidence`, and a group:
/// - `rel_a`: source `report`, target `testing`, holding the Property
///   `rel_a_p`;
/// - `rel_b`: source `rel_a`, target `tester`;
/// - `rel_c`: source `tester`, target `rel_b`;
/// - `rel_d`: source `testing`, target `tester`;
/// - the `ArtifactGroup` `g2`, members `report rel_a tester`.
fn chain() -> String {
    case(
        &format!(
            "{}{}{}{}{}",
            rel(
                "rel_a",
                " source=\"report\" target=\"testing\"",
                "<property xmi:id=\"rel_a_p\"><name content=\"P\"/></property>"
            ),
            rel("rel_b", " source=\"rel_a\" target=\"tester\"", ""),
            rel("rel_c", " source=\"tester\" target=\"rel_b\"", ""),
            rel("rel_d", " source=\"testing\" target=\"tester\"", ""),
            "<artifactElement xmi:type=\"sacm:ArtifactGroup\" xmi:id=\"g2\" \
             artifactElement=\"report rel_a tester\"><name content=\"G2\"/></artifactElement>"
        ),
        "",
    )
}

#[test]
fn sacm23_art_010_delete_referencing_relationships_deletes_relationships_to_a_fixpoint() {
    let before = read_ok("the chain", &chain());
    for (delete, deleted, members) in [
        // report is rel_a's source; rel_a is rel_b's source; rel_b is rel_c's
        // target. rel_d names none of them.
        (
            "report",
            vec!["report", "pr1", "rel_a", "rel_a_p", "rel_b", "rel_c"],
            vec!["tester"],
        ),
        // testing is rel_a's target and rel_d's source.
        (
            "testing",
            vec!["testing", "rel_a", "rel_a_p", "rel_b", "rel_c", "rel_d"],
            vec!["report", "tester"],
        ),
    ] {
        let what = format!("deleting {delete} under DeleteReferencingRelationships");
        let mut doc = before.clone();
        let mutation = applied(
            &mut doc,
            DeleteElement::new(delete)
                .with_package_policy(PackageDeletePolicy::DeleteRecursively)
                .with_reference_policy(ReferenceDeletePolicy::DeleteReferencingRelationships),
        );
        assert_eq!(
            (strs(mutation.deleted_ids()), strs(mutation.changed_ids())),
            (deleted, vec!["g2"]),
            "{ROW}: {what}: deleted and changed ids"
        );
        assert_eq!(
            strs(element(&doc, "g2").group_members()),
            members,
            "{ROW}: {what}: the group keeps existing with the other members"
        );
        assert_clean_delete(&what, &before, &doc, &mutation);
        round_trip(&what, &doc, Mode::Strict);
    }
}

#[test]
fn sacm23_art_010_scrub_references_deletes_a_relationship_only_when_an_end_empties() {
    let scrub = |doc: &mut Document, delete: &str| {
        let before = doc.clone();
        let mutation = applied(
            doc,
            DeleteElement::new(delete)
                .with_package_policy(PackageDeletePolicy::DeleteRecursively)
                .with_reference_policy(ReferenceDeletePolicy::ScrubReferences),
        );
        assert_clean_delete(&format!("scrubbing {delete}"), &before, doc, &mutation);
        mutation
    };
    let input = case(
        &format!(
            "{}{}",
            rel(
                "rel_a",
                " source=\"report fta\" target=\"testing\"",
                "<property xmi:id=\"rel_a_p\"><name content=\"P\"/></property>"
            ),
            rel("rel_b", " source=\"rel_a\" target=\"tester testing\"", ""),
        ),
        "",
    );
    // The sources: rel_a keeps fta, then loses its last source and goes, and
    // rel_b, whose only source it is, with it.
    let mut doc = read_ok("two relationships", &input);
    let mutation = scrub(&mut doc, "report");
    assert_eq!(
        (
            strs(mutation.deleted_ids()),
            strs(mutation.changed_ids()),
            ends_of(&doc, "rel_a")
        ),
        (
            vec!["report", "pr1"],
            vec!["rel_a"],
            (owned(&["fta"]), owned(&["testing"]))
        ),
        "{ROW}: scrubbing report, one of two sources"
    );
    let mutation = scrub(&mut doc, "fta");
    assert_eq!(
        (strs(mutation.deleted_ids()), strs(mutation.changed_ids())),
        (vec!["fta", "rel_a", "rel_a_p", "rel_b"], Vec::<&str>::new()),
        "{ROW}: scrubbing fta, the last source of rel_a"
    );
    // The targets: rel_b keeps testing, then loses its last target and goes;
    // rel_a loses its only target too.
    let mut doc = read_ok("two relationships", &input);
    let mutation = scrub(&mut doc, "tester");
    assert_eq!(
        (
            strs(mutation.deleted_ids()),
            strs(mutation.changed_ids()),
            ends_of(&doc, "rel_b")
        ),
        (
            vec!["tester"],
            vec!["rel_b"],
            (owned(&["rel_a"]), owned(&["testing"]))
        ),
        "{ROW}: scrubbing tester, one of two targets"
    );
    let mutation = scrub(&mut doc, "testing");
    assert_eq!(
        (strs(mutation.deleted_ids()), strs(mutation.changed_ids())),
        (
            vec!["testing", "rel_a", "rel_a_p", "rel_b"],
            Vec::<&str>::new()
        ),
        "{ROW}: scrubbing testing, the last target of both"
    );
    round_trip("after the scrubs", &doc, Mode::Strict);
}

#[test]
fn sacm23_art_010_referrers_lists_both_ends() {
    let doc = read_ok(
        "two relationships",
        &case(
            &format!(
                "{}{}",
                rel(
                    "rel",
                    " source=\"report tester\" target=\"testing tester\"",
                    ""
                ),
                rel("rel2", " source=\"rel\" target=\"tester\"", "")
            ),
            "",
        ),
    );
    let referrers = |id: &str| -> Vec<(String, ReferenceEnd, usize)> {
        doc.referrers(id)
            .iter()
            .map(|referrer| {
                (
                    referrer.holder().id().as_str().to_owned(),
                    referrer.end(),
                    referrer.position(),
                )
            })
            .collect()
    };
    assert_eq!(
        referrers("tester"),
        [
            ("rel".to_owned(), ReferenceEnd::ArtifactSource, 1),
            ("rel".to_owned(), ReferenceEnd::ArtifactTarget, 1),
            ("rel2".to_owned(), ReferenceEnd::ArtifactTarget, 0),
        ],
        "{ROW}: the referrers of tester"
    );
    assert_eq!(
        (referrers("report"), referrers("rel")),
        (
            vec![("rel".to_owned(), ReferenceEnd::ArtifactSource, 0)],
            vec![("rel2".to_owned(), ReferenceEnd::ArtifactSource, 0)]
        ),
        "{ROW}: the referrers of report and of rel"
    );
}

// =====================================================================================
// Item 10: the other edits
// =====================================================================================

#[test]
fn sacm23_art_010_the_element_edits_apply_to_a_relationship() {
    let mut doc = read_ok("a relationship", &with_rel(""));
    applied(
        &mut doc,
        SetName::new("rel", LangString::new("Produced by")),
    );
    applied(&mut doc, SetGid::new("rel", "REL-9"));
    applied(&mut doc, SetFlag::new("rel", Flag::IsCitation, true));
    applied(&mut doc, SetFlag::new("rel", Flag::IsAbstract, true));
    applied(
        &mut doc,
        SetReferences::new("rel", ReferenceEnd::CitedElement, ["report"]),
    );
    applied(
        &mut doc,
        CreateProperty::new("rel", LangString::new("Reviewed")).with_id("rp1"),
    );
    let found = element(&doc, "rel");
    assert_eq!(
        (
            found.name().cloned(),
            found.gid(),
            found.is_citation(),
            found.is_abstract(),
            found.cited_element().map(ElementId::as_str),
            found
                .properties()
                .map(|property| property.id().as_str().to_owned())
                .collect::<Vec<_>>(),
        ),
        (
            Some(LangString::new("Produced by")),
            Some("REL-9"),
            true,
            true,
            Some("report"),
            vec!["rp1".to_owned()],
        ),
        "{ROW}: after SetName, SetGid, SetFlag, SetReferences and CreateProperty"
    );
    // isCounter is an AssertedRelationship's flag.
    let error = refuse(&mut doc, SetFlag::new("rel", Flag::IsCounter, true));
    assert!(
        matches!(
            &error,
            EditError::FeatureNotApplicable { element, kind: ElementKind::ArtifactAssetRelationship, feature: "isCounter", .. }
                if element.as_str() == "rel"
        ),
        "{ROW}: SetFlag(IsCounter): {error:?}"
    );
    // Reordered among the package's children.
    let mut order = children(&doc, "evidence");
    order.rotate_right(1);
    assert_eq!(
        order.first().map(String::as_str),
        Some("rel"),
        "{ROW}: setup"
    );
    applied(&mut doc, ReorderElements::new("evidence", order.clone()));
    assert_eq!(
        children(&doc, "evidence"),
        order,
        "{ROW}: the relationship is not reordered first"
    );
    round_trip("after the edits", &doc, Mode::Strict);
    // Deleted, with nothing referencing it.
    let before = doc.clone();
    let mutation = applied(
        &mut doc,
        DeleteElement::new("rel").with_package_policy(PackageDeletePolicy::DeleteRecursively),
    );
    assert_eq!(
        strs(mutation.deleted_ids()),
        ["rel", "rp1"],
        "{ROW}: deleting the relationship"
    );
    assert_clean_delete("deleting the relationship", &before, &doc, &mutation);
}

#[test]
fn sacm23_art_010_move_and_copy_apply_and_a_copy_with_an_empty_end_is_too_few_references() {
    let mut doc = read_ok("a relationship", &with_rel(""));
    // Moved: the only legal parent is an ArtifactPackage.
    applied(&mut doc, MoveElement::new("rel", "other"));
    assert_eq!(
        children(&doc, "other"),
        ["rel"],
        "{ROW}: the relationship is not moved into other"
    );
    for (parent, parent_kind) in [
        ("p", ElementKind::ArgumentPackage),
        ("case", ElementKind::AssuranceCasePackage),
        ("report", ElementKind::Artifact),
        ("grp", ElementKind::ArtifactGroup),
    ] {
        let error = refuse(&mut doc, MoveElement::new("rel", parent));
        assert!(
            matches!(
                &error,
                EditError::InvalidParent { parent: p, parent_kind: k, kind: ElementKind::ArtifactAssetRelationship, .. }
                    if p.as_str() == parent && *k == parent_kind
            ),
            "{ROW}: moving the relationship under the {parent_kind} {parent}: {error:?}"
        );
    }
    // Copied: its ends name the elements it names, which are not copied.
    let source = doc.clone();
    let copy = match CopyElements::new(&source, ["rel"], "evidence") {
        Ok(copy) => copy,
        Err(error) => panic!("{ROW}: CopyElements::new refused the relationship: {error}"),
    };
    let mutation = applied(&mut doc, copy);
    let [copied] = mutation.created_ids() else {
        panic!("{ROW}: the copy created {:?}", mutation.created_ids());
    };
    let found = element(&doc, copied.as_str());
    assert_eq!(
        (
            found.kind(),
            found.parent().map(|parent| parent.id().as_str().to_owned()),
            strs(found.sources())
        ),
        (
            ElementKind::ArtifactAssetRelationship,
            Some("evidence".to_owned()),
            vec!["report"]
        ),
        "{ROW}: the copy"
    );
    round_trip("after the move and the copy", &doc, Mode::Strict);
    // A relationship with no source or no target is not copied.
    for (attributes, end) in [
        (" target=\"testing\"", "source"),
        (" source=\"report\"", "target"),
    ] {
        let source = read_ok(
            "a relationship with an empty end",
            &case(&rel("rel", attributes, ""), ""),
        );
        let copy = match CopyElements::new(&source, ["rel"], "other") {
            Ok(copy) => copy,
            Err(error) => panic!("{ROW}: CopyElements::new refused the relationship: {error}"),
        };
        let mut target = source.clone();
        let error = refuse(&mut target, copy);
        assert!(
            matches!(
                &error,
                EditError::TooFewReferences { end: e, given: 0, required: 1, .. } if *e == end
            ),
            "{ROW}: copying a relationship with no {end}: {error:?}"
        );
    }
}

#[test]
fn sacm23_art_010_deleting_a_relationship_holding_a_property_is_not_empty_unless_recursive() {
    let mut doc = read_ok(
        "a relationship holding a Property",
        &case(
            &rel(
                "rel",
                " source=\"report\" target=\"testing\"",
                "<property xmi:id=\"rp1\"><name content=\"RP1\"/></property>",
            ),
            "",
        ),
    );
    let error = refuse(&mut doc, DeleteElement::new("rel"));
    assert!(
        matches!(
            &error,
            EditError::NotEmpty { element, contained: 1, .. } if element.as_str() == "rel"
        ),
        "{ROW}: deleting the relationship: not NotEmpty {{ rel, 1 }}: {error:?}"
    );
    assert_eq!(
        error.code(),
        Code::CMD_PACKAGE_NOT_EMPTY,
        "{ROW}: not SACM-CMD-006"
    );
    let before = doc.clone();
    let mutation = applied(
        &mut doc,
        DeleteElement::new("rel").with_package_policy(PackageDeletePolicy::DeleteRecursively),
    );
    assert_eq!(
        strs(mutation.deleted_ids()),
        ["rel", "rp1"],
        "{ROW}: a recursive delete of the relationship"
    );
    assert_clean_delete("a recursive delete", &before, &doc, &mutation);
    // CrossPackageReferencePolicy applies to its ends as to any reference.
    let mut doc = read_ok(
        "a relationship in other",
        &case(
            "",
            &rel("rel2", " source=\"report fta\" target=\"tester\"", ""),
        ),
    );
    let before = doc.clone();
    let mutation = applied(
        &mut doc,
        DeleteElement::new("report")
            .with_package_policy(PackageDeletePolicy::DeleteRecursively)
            .with_reference_policy(ReferenceDeletePolicy::ScrubReferences)
            .with_cross_package_policy(CrossPackageReferencePolicy::ApplyReferencePolicy),
    );
    assert_eq!(
        (strs(mutation.deleted_ids()), strs(mutation.changed_ids())),
        (vec!["report", "pr1"], vec!["rel2"]),
        "{ROW}: scrubbing report from rel2 across packages"
    );
    assert_clean_delete("scrubbing across packages", &before, &doc, &mutation);
}
