//! Tests for the API slice 0052 adds for the Artifact package (ADR 0034):
//! the five creates `CreateArtifactPackage` (SACM23-ART-001),
//! `CreateProperty` (SACM23-ART-003), `CreateTechnique` (SACM23-ART-008),
//! `CreateParticipant` (SACM23-ART-009) and `CreateArtifactGroup`
//! (SACM23-ART-011); `SacmElement::properties()` (SACM23-ART-003); and
//! `ReferenceEnd::ArtifactGroupMember` with its edits and
//! `Difference::ArtifactGroupMembers` (SACM23-ART-011).
//!
//! Each create runs ADR 0019's checks in order (`ParentNotFound`,
//! `InvalidParent`, `InvalidId`, `DuplicateId`); the group's members then get
//! ADR 0028's reference checks (`InvalidReference`, then `ReferenceNotFound`,
//! then `WrongReferenceType` with `expected: "ArtifactElement"`, each over every
//! member in order, as for an `ArgumentGroup`). A refusal leaves the document
//! `==` to before, and `edit::check` and `edit::preview` answer as
//! `edit::apply` does.

use sacm::compare::{Difference, semantic_compare};
use sacm::diagnostic::{Code, Diagnostic};
use sacm::edit::{
    self, AddReference, CreateArgumentGroup, CreateArgumentPackage, CreateArtifactGroup,
    CreateArtifactPackage, CreateAssuranceCasePackage, CreateClaim, CreateDescription,
    CreateParticipant, CreateProperty, CreateTechnique, EditError, Mutation, Operation,
    RemoveReference, SetReferences,
};
use sacm::model::{AbstractClass, Document, ElementId, ElementKind, LangString, ReferenceEnd};
use sacm::validation;
use sacm::xmi::{self, Mode, ReadOptions, WriteOptions};

const ART_001: &str = "SACM23-ART-001";
const ART_003: &str = "SACM23-ART-003";
const ART_008: &str = "SACM23-ART-008";
const ART_009: &str = "SACM23-ART-009";
const ART_011: &str = "SACM23-ART-011";

/// The group end's name, as in ptc/22-03-13.
const END: &str = "artifactElement";

/// The worked fixture of slice 0052.
const FIXTURE: &str = concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/tests/data/sacm23/xmi/artifact-package-valid.sacm.xmi"
);

// --- Helpers -----------------------------------------------------------------------

fn named(content: &str) -> LangString {
    LangString::new(content).with_lang("en")
}

fn list(values: &[&str]) -> Vec<ElementId> {
    values.iter().copied().map(ElementId::from).collect()
}

fn strs(values: &[ElementId]) -> Vec<&str> {
    values.iter().map(ElementId::as_str).collect()
}

/// Applies `operation`, which the test expects to succeed, after checking
/// that `edit::check` accepts it and `edit::preview` gives the mutation
/// `apply` gives.
fn applied(row: &str, doc: &mut Document, operation: impl Into<Operation>) -> Mutation {
    let operation = operation.into();
    let checked = edit::check(doc, &operation);
    let previewed = edit::preview(doc, &operation).map(|preview| preview.mutation().clone());
    let mutation = match edit::apply(doc, operation.clone()) {
        Ok(mutation) => mutation,
        Err(error) => panic!("{row}: {operation:?} was refused: {error} ({error:?})"),
    };
    assert_eq!(
        checked,
        Ok(()),
        "{row}: check refuses {operation:?}, which apply accepts"
    );
    assert_eq!(
        previewed.as_ref(),
        Ok(&mutation),
        "{row}: preview of {operation:?} does not give apply's mutation"
    );
    mutation
}

/// Applies a create the test expects to succeed, and returns the one id it
/// created, with nothing changed or deleted.
fn create(row: &str, doc: &mut Document, operation: impl Into<Operation>) -> ElementId {
    let mutation = applied(row, doc, operation);
    let [only] = mutation.created_ids() else {
        panic!(
            "{row}: created_ids() is {:?}, not the one new element",
            mutation.created_ids()
        );
    };
    assert_eq!(
        (mutation.changed_ids(), mutation.deleted_ids()),
        ([].as_slice(), [].as_slice()),
        "{row}: a create changed or deleted elements"
    );
    only.clone()
}

/// Applies `operation`, which the test expects to be refused leaving `doc`
/// `==` to before, with `check` and `preview` refusing it the same, and
/// returns why.
fn refuse(row: &str, doc: &mut Document, operation: impl Into<Operation>) -> EditError {
    let operation = operation.into();
    let before = doc.clone();
    let checked = edit::check(doc, &operation);
    let previewed = edit::preview(doc, &operation).map(|preview| preview.mutation().clone());
    let error = match edit::apply(doc, operation.clone()) {
        Ok(mutation) => panic!("{row}: {operation:?} was accepted: {mutation:?}"),
        Err(error) => error,
    };
    assert!(
        *doc == before,
        "{row}: the refused {operation:?} changed the document"
    );
    assert_eq!(
        (checked, previewed),
        (Err(error.clone()), Err(error.clone())),
        "{row}: check or preview answer {operation:?} otherwise than apply"
    );
    let message = error.to_string();
    assert!(
        !message.contains('\n') && message.starts_with(char::is_lowercase),
        "{row}: the message {message:?} is not one line starting in lower case"
    );
    error
}

/// A document built by edits:
/// - the `AssuranceCasePackage` `case`, holding the `ArgumentPackage` `p` (the
///   Claim `c1` and the `ArgumentGroup` `ag1`) and the `ArtifactPackage`
///   `evidence`;
/// - `evidence` holds the Technique `t1` (the Property `pr1` and the
///   Description `d1`), the Participant `pa1` and the `ArtifactGroup` `g1`;
/// - the `ArtifactPackage` root `shared`.
fn built() -> Document {
    let row = "SACM23-ART-001";
    let mut doc = Document::new();
    create(
        row,
        &mut doc,
        CreateAssuranceCasePackage::root(named("Case")).with_id("case"),
    );
    create(
        row,
        &mut doc,
        CreateArgumentPackage::new("case", named("P")).with_id("p"),
    );
    create(
        row,
        &mut doc,
        CreateClaim::new("p", named("C1")).with_id("c1"),
    );
    create(
        row,
        &mut doc,
        CreateArgumentGroup::new("p", named("AG1")).with_id("ag1"),
    );
    create(
        row,
        &mut doc,
        CreateArtifactPackage::new("case", named("Evidence")).with_id("evidence"),
    );
    create(
        row,
        &mut doc,
        CreateTechnique::new("evidence", named("FTA")).with_id("t1"),
    );
    create(
        row,
        &mut doc,
        CreateProperty::new("t1", named("Qualified")).with_id("pr1"),
    );
    create(
        row,
        &mut doc,
        CreateDescription::new("t1", named("Top-down")).with_id("d1"),
    );
    create(
        row,
        &mut doc,
        CreateParticipant::new("evidence", named("Assessor")).with_id("pa1"),
    );
    create(
        row,
        &mut doc,
        CreateArtifactGroup::new("evidence", named("G1")).with_id("g1"),
    );
    create(
        row,
        &mut doc,
        CreateArtifactPackage::root(named("Shared")).with_id("shared"),
    );
    doc
}

fn element<'d>(row: &str, doc: &'d Document, id: &str) -> sacm::model::SacmElement<'d> {
    let Some(found) = doc.get(id) else {
        panic!("{row}: the document has no element {id:?}");
    };
    found
}

/// `(kind, parent id, name)` of `id`.
fn placed(
    row: &str,
    doc: &Document,
    id: &str,
) -> (ElementKind, Option<String>, Option<LangString>) {
    let found = element(row, doc, id);
    (
        found.kind(),
        found.parent().map(|parent| parent.id().as_str().to_owned()),
        found.name().cloned(),
    )
}

fn write_strict(row: &str, doc: &Document) -> String {
    match xmi::write(doc, &WriteOptions::default().with_mode(Mode::Strict)) {
        Ok(text) => text,
        Err(error) => panic!("{row}: a strict write refused the document: {error:?}"),
    }
}

fn read_back(row: &str, text: &str) -> Document {
    match xmi::read(text.as_bytes(), &ReadOptions::default()) {
        Ok(output) => {
            assert!(
                output.diagnostics().is_empty(),
                "{row}: reading the output reported {:?}",
                output.diagnostics()
            );
            output.into_document()
        }
        Err(error) => panic!("{row}: the output does not read back: {error}\n{text}"),
    }
}

fn line_with<'t>(row: &str, text: &'t str, id: &str) -> &'t str {
    let attribute = format!("xmi:id=\"{id}\"");
    let Some(line) = text.lines().find(|line| line.contains(&attribute)) else {
        panic!("{row}: the output has no line with {attribute}:\n{text}");
    };
    line.trim()
}

/// The refusal named by its variant and the ids it names, comparable.
fn refusal(error: &EditError) -> (String, Vec<String>) {
    let shown = format!("{error:?}");
    let variant = shown
        .split([' ', '{', '('])
        .next()
        .unwrap_or_default()
        .to_owned();
    let named = match error {
        EditError::ParentNotFound { parent, .. } => vec![parent.as_str().to_owned()],
        EditError::InvalidParent {
            parent,
            parent_kind,
            kind,
            ..
        } => vec![
            parent.as_str().to_owned(),
            parent_kind.name().to_owned(),
            kind.name().to_owned(),
        ],
        EditError::InvalidId { id, .. } | EditError::DuplicateId { id, .. } => {
            vec![id.as_str().to_owned()]
        }
        _ => Vec::new(),
    };
    (variant, named)
}

/// One create, as a function of its parent (`None` for a root, where the
/// create allows one) and its caller id.
type Make = fn(Option<&str>, Option<&str>) -> Operation;

/// Asserts the creates of `kind` by `make` on [`built`]: created under each
/// of `legal` (with the role written as `written_role`), the generated ids
/// `<prefix>_1` and `<prefix>_2`, `InvalidParent` under each of `illegal`,
/// and the four refusals in order.
#[expect(
    clippy::too_many_lines,
    reason = "the legal parents, the illegal ones and the check order of one create read best as one list"
)]
fn assert_create_rules(
    row: &str,
    kind: ElementKind,
    make: Make,
    prefix: &str,
    legal: &[(&str, &str)],
    illegal: &[&str],
) {
    let mut doc = built();
    // Generated ids, under the first legal parent.
    let first = Some(legal[0].0);
    let id1 = create(row, &mut doc, make(first, None));
    let id2 = create(row, &mut doc, make(first, None));
    assert_eq!(
        (id1.as_str(), id2.as_str()),
        (
            format!("{prefix}_1").as_str(),
            format!("{prefix}_2").as_str()
        ),
        "{row}: the generated ids of {kind}"
    );
    assert_eq!(
        placed(row, &doc, id1.as_str()),
        (kind, Some(legal[0].0.to_owned()), Some(named("N"))),
        "{row}: the created {kind}"
    );
    // Every legal parent, with a caller id; the role it is written in.
    for (index, (parent, role)) in legal.iter().enumerate() {
        let given = format!("given_{index}");
        let id = create(row, &mut doc, make(Some(*parent), Some(&given)));
        assert_eq!(
            (id.as_str(), placed(row, &doc, &given)),
            (
                given.as_str(),
                (kind, Some((*parent).to_owned()), Some(named("N")))
            ),
            "{row}: a {kind} with a caller id under {parent}"
        );
        let last = element(row, &doc, parent).children().last();
        assert_eq!(
            last.map(|child| child.id().as_str().to_owned()),
            Some(given.clone()),
            "{row}: the {kind} under {parent} is not its last child"
        );
        let text = write_strict(row, &doc);
        let tag = line_with(row, &text, &given);
        assert!(
            tag.starts_with(&format!("<{role} ")),
            "{row}: the {kind} under {parent} is not written in {role}: {tag}"
        );
    }
    assert_eq!(
        validation::validate(&doc),
        [],
        "{row}: the built document does not validate cleanly"
    );
    let back = read_back(row, &write_strict(row, &doc));
    assert!(back == doc, "{row}: the built document reads back !=");
    // InvalidParent under each illegal parent.
    for parent in illegal {
        let parent_kind = element(row, &doc, parent).kind();
        let error = refuse(row, &mut doc, make(Some(*parent), None));
        assert_eq!(
            refusal(&error),
            (
                "InvalidParent".to_owned(),
                vec![
                    (*parent).to_owned(),
                    parent_kind.name().to_owned(),
                    kind.name().to_owned()
                ]
            ),
            "{row}: a {kind} under the {parent_kind} {parent}"
        );
        assert_eq!(
            error.code(),
            Code::CMD_INVALID_PARENT,
            "{row}: not SACM-CMD-005"
        );
    }
    // The four checks in order: each later fault is hidden by an earlier.
    let legal_parent = Some(legal[0].0);
    let illegal_parent = Some(illegal[0]);
    for (what, parent, id, expected) in [
        (
            "a missing parent, an invalid id",
            Some("nowhere"),
            Some("1x"),
            ("ParentNotFound", vec!["nowhere"]),
        ),
        (
            "a missing parent, a duplicate id",
            Some("nowhere"),
            Some("c1"),
            ("ParentNotFound", vec!["nowhere"]),
        ),
        (
            "an illegal parent, an invalid id",
            illegal_parent,
            Some("1x"),
            (
                "InvalidParent",
                vec![
                    illegal[0],
                    element(row, &doc, illegal[0]).kind().name(),
                    kind.name(),
                ],
            ),
        ),
        (
            "an illegal parent, a duplicate id",
            illegal_parent,
            Some("c1"),
            (
                "InvalidParent",
                vec![
                    illegal[0],
                    element(row, &doc, illegal[0]).kind().name(),
                    kind.name(),
                ],
            ),
        ),
        (
            "an invalid id",
            legal_parent,
            Some("1x"),
            ("InvalidId", vec!["1x"]),
        ),
        (
            "an id with a space",
            legal_parent,
            Some("a b"),
            ("InvalidId", vec!["a b"]),
        ),
        (
            "a duplicate id",
            legal_parent,
            Some("c1"),
            ("DuplicateId", vec!["c1"]),
        ),
        (
            "the id of the parent",
            legal_parent,
            legal_parent,
            ("DuplicateId", vec![legal[0].0]),
        ),
    ] {
        let error = refuse(row, &mut doc, make(parent, id));
        assert_eq!(
            refusal(&error),
            (
                expected.0.to_owned(),
                expected.1.iter().map(|&s| s.to_owned()).collect::<Vec<_>>()
            ),
            "{row}: {what}: the refusal of a {kind}"
        );
    }
    // A refusal does not move the next generated id.
    let next = create(row, &mut doc, make(first, None));
    assert_eq!(
        next.as_str(),
        format!("{prefix}_3"),
        "{row}: a refusal moved the next generated id of {kind}"
    );
}

// =====================================================================================
// SACM23-ART-001 item 5: CreateArtifactPackage
// =====================================================================================

#[test]
fn sacm23_art_001_create_artifact_package_reports_what_it_was_given() {
    let root = CreateArtifactPackage::root(named("Evidence"));
    assert_eq!(
        (root.parent(), root.id(), root.name()),
        (None, None, &named("Evidence")),
        "{ART_001}: root(name)"
    );
    let child = CreateArtifactPackage::new("case", named("Evidence")).with_id("evidence");
    assert_eq!(
        (
            child.parent().map(ElementId::as_str),
            child.id().map(ElementId::as_str),
            child.name()
        ),
        (Some("case"), Some("evidence"), &named("Evidence")),
        "{ART_001}: new(parent, name).with_id(id)"
    );
    assert_eq!(
        (
            Operation::from(root).operation_name(),
            Operation::from(child).operation_name()
        ),
        ("CreateArtifactPackage", "CreateArtifactPackage"),
        "{ART_001}: operation_name()"
    );
}

#[test]
fn sacm23_art_001_create_artifact_package_creates_roots_in_document_order() {
    let mut doc = Document::new();
    let first = create(
        ART_001,
        &mut doc,
        CreateArtifactPackage::root(named("Evidence")),
    );
    let second = create(
        ART_001,
        &mut doc,
        CreateArtifactPackage::root(named("More")).with_id("more"),
    );
    let third = create(
        ART_001,
        &mut doc,
        CreateArtifactPackage::root(LangString::default()),
    );
    assert_eq!(
        (
            strs(&[first, second, third]),
            doc.roots()
                .map(|root| (root.id().as_str().to_owned(), root.kind()))
                .collect::<Vec<_>>()
        ),
        (
            vec!["artifact_package_1", "more", "artifact_package_2"],
            vec![
                (
                    "artifact_package_1".to_owned(),
                    ElementKind::ArtifactPackage
                ),
                ("more".to_owned(), ElementKind::ArtifactPackage),
                (
                    "artifact_package_2".to_owned(),
                    ElementKind::ArtifactPackage
                )
            ]
        ),
        "{ART_001}: the created roots"
    );
    assert_eq!(
        element(ART_001, &doc, "artifact_package_2").name(),
        Some(&LangString::default()),
        "{ART_001}: the name is not stored exactly"
    );
    let text = match xmi::write(&doc, &WriteOptions::default().with_mode(Mode::Tolerant)) {
        Ok(text) => text,
        Err(error) => panic!("{ART_001}: a tolerant write refused the roots: {error:?}"),
    };
    assert_eq!(
        line_with(ART_001, &text, "more"),
        "<sacm:ArtifactPackage xmi:id=\"more\">",
        "{ART_001}: a created root is not written as sacm:ArtifactPackage\n{text}"
    );
}

#[test]
fn sacm23_art_001_create_artifact_package_legal_parents_refusals_and_generated_id() {
    assert_create_rules(
        ART_001,
        ElementKind::ArtifactPackage,
        |parent, id| {
            let create = match parent {
                Some(parent) => CreateArtifactPackage::new(parent, named("N")),
                None => CreateArtifactPackage::root(named("N")),
            };
            match id {
                Some(id) => create.with_id(id).into(),
                None => create.into(),
            }
        },
        "artifact_package",
        &[
            ("case", "artifactPackage"),
            ("evidence", "artifactElement"),
            ("shared", "artifactElement"),
        ],
        &["p", "c1", "t1", "g1"],
    );
}

// =====================================================================================
// SACM23-ART-003: properties() and CreateProperty
// =====================================================================================

#[test]
fn sacm23_art_003_properties_lists_an_asset_s_property_children_and_nothing_else() {
    let text = std::fs::read_to_string(FIXTURE)
        .unwrap_or_else(|error| panic!("{ART_003}: cannot read {FIXTURE}: {error}"));
    let doc = match xmi::read(text.as_bytes(), &ReadOptions::default()) {
        Ok(output) => output.into_document(),
        Err(error) => panic!("{ART_003}: the fixture does not read: {error}"),
    };
    let properties = |id: &str| -> Vec<String> {
        element(ART_003, &doc, id)
            .properties()
            .map(|property| property.id().as_str().to_owned())
            .collect()
    };
    assert_eq!(properties("fta"), ["fta_q"], "{ART_003}: the fixture's fta");
    // A Property held directly in an ArtifactPackage is a child of it, not a
    // property of it.
    assert_eq!(
        (
            properties("library"),
            element(ART_003, &doc, "library")
                .children()
                .map(|child| child.id().as_str().to_owned())
                .collect::<Vec<_>>()
        ),
        (Vec::<String>::new(), vec!["calibrated".to_owned()]),
        "{ART_003}: the fixture's library"
    );
    for id in [
        "case",
        "evidence",
        "fta_q",
        "assessor",
        "analysis",
        "calibrated",
        "shared",
        "supplier",
    ] {
        assert_eq!(
            properties(id),
            Vec::<String>::new(),
            "{ART_003}: the fixture's {id} has properties"
        );
    }
    // In order, among utility children, on each readable asset kind.
    let mut doc = built();
    create(
        ART_003,
        &mut doc,
        CreateProperty::new("t1", named("Second")).with_id("pr2"),
    );
    create(
        ART_003,
        &mut doc,
        CreateProperty::new("pr2", named("Inner")).with_id("pr3"),
    );
    create(
        ART_003,
        &mut doc,
        CreateProperty::new("pa1", named("P")).with_id("pr4"),
    );
    create(
        ART_003,
        &mut doc,
        CreateProperty::new("evidence", named("P")).with_id("pr5"),
    );
    let properties = |id: &str| -> Vec<String> {
        element(ART_003, &doc, id)
            .properties()
            .map(|property| property.id().as_str().to_owned())
            .collect()
    };
    assert_eq!(
        (
            properties("t1"),
            properties("pr2"),
            properties("pr3"),
            properties("pa1"),
            properties("evidence"),
            properties("g1"),
            properties("c1")
        ),
        (
            vec!["pr1".to_owned(), "pr2".to_owned()],
            vec!["pr3".to_owned()],
            Vec::new(),
            vec!["pr4".to_owned()],
            Vec::new(),
            Vec::new(),
            Vec::new()
        ),
        "{ART_003}: properties() of t1, pr2, pr3, pa1, evidence, g1 and c1"
    );
    assert!(
        element(ART_003, &doc, "t1")
            .properties()
            .all(|property| property.kind() == ElementKind::Property),
        "{ART_003}: properties() yields something other than a Property"
    );
}

#[test]
fn sacm23_art_003_create_property_legal_parents_refusals_and_generated_id() {
    assert_create_rules(
        ART_003,
        ElementKind::Property,
        |parent, id| {
            let create = CreateProperty::new(parent.unwrap_or("evidence"), named("N"));
            match id {
                Some(id) => create.with_id(id).into(),
                None => create.into(),
            }
        },
        "property",
        &[
            ("t1", "property"),
            ("pr1", "property"),
            ("pa1", "property"),
            ("evidence", "artifactElement"),
            ("shared", "artifactElement"),
        ],
        &["c1", "g1", "case", "p"],
    );
    let create = CreateProperty::new("t1", named("Q")).with_id("q");
    assert_eq!(
        (
            create.parent().as_str(),
            create.id().map(ElementId::as_str),
            create.name(),
            Operation::from(create.clone()).operation_name()
        ),
        ("t1", Some("q"), &named("Q"), "CreateProperty"),
        "{ART_003}: CreateProperty reports what it was given"
    );
}

// =====================================================================================
// SACM23-ART-008 / SACM23-ART-009 item 2: CreateTechnique, CreateParticipant
// =====================================================================================

#[test]
fn sacm23_art_008_create_technique_legal_parents_refusals_and_generated_id() {
    assert_create_rules(
        ART_008,
        ElementKind::Technique,
        |parent, id| {
            let create = CreateTechnique::new(parent.unwrap_or("evidence"), named("N"));
            match id {
                Some(id) => create.with_id(id).into(),
                None => create.into(),
            }
        },
        "technique",
        &[
            ("evidence", "artifactElement"),
            ("shared", "artifactElement"),
        ],
        &["case", "p", "t1", "g1", "pr1", "c1"],
    );
    let create = CreateTechnique::new("evidence", named("T")).with_id("t");
    assert_eq!(
        (
            create.parent().as_str(),
            create.id().map(ElementId::as_str),
            create.name(),
            Operation::from(create.clone()).operation_name()
        ),
        ("evidence", Some("t"), &named("T"), "CreateTechnique"),
        "{ART_008}: CreateTechnique reports what it was given"
    );
}

#[test]
fn sacm23_art_009_create_participant_legal_parents_refusals_and_generated_id() {
    assert_create_rules(
        ART_009,
        ElementKind::Participant,
        |parent, id| {
            let create = CreateParticipant::new(parent.unwrap_or("evidence"), named("N"));
            match id {
                Some(id) => create.with_id(id).into(),
                None => create.into(),
            }
        },
        "participant",
        &[
            ("evidence", "artifactElement"),
            ("shared", "artifactElement"),
        ],
        &["case", "p", "t1", "g1", "pa1", "c1"],
    );
    let create = CreateParticipant::new("evidence", named("P")).with_id("pa");
    assert_eq!(
        (
            create.parent().as_str(),
            create.id().map(ElementId::as_str),
            create.name(),
            Operation::from(create.clone()).operation_name()
        ),
        ("evidence", Some("pa"), &named("P"), "CreateParticipant"),
        "{ART_009}: CreateParticipant reports what it was given"
    );
}

// =====================================================================================
// SACM23-ART-011: CreateArtifactGroup
// =====================================================================================

#[test]
fn sacm23_art_011_create_artifact_group_reports_what_it_was_given() {
    let create = CreateArtifactGroup::new("evidence", named("Group"));
    assert_eq!(
        (
            create.parent().as_str(),
            create.id(),
            create.name(),
            create.group_members()
        ),
        ("evidence", None, &named("Group"), [].as_slice()),
        "{ART_011}: the defaults"
    );
    let create = create
        .with_id("given")
        .with_group_members(["pa1", "t1"])
        .with_group_members(["t1", "pa1", "t1"]);
    assert_eq!(
        (
            create.id().map(ElementId::as_str),
            strs(create.group_members())
        ),
        (Some("given"), vec!["t1", "pa1", "t1"]),
        "{ART_011}: with_id, and with_group_members replacing what was given before"
    );
    assert_eq!(
        Operation::from(create).operation_name(),
        "CreateArtifactGroup",
        "{ART_011}: operation_name()"
    );
}

#[test]
fn sacm23_art_011_create_artifact_group_legal_parents_refusals_and_generated_id() {
    assert_create_rules(
        ART_011,
        ElementKind::ArtifactGroup,
        |parent, id| {
            let create = CreateArtifactGroup::new(parent.unwrap_or("evidence"), named("N"))
                .with_group_members(["t1"]);
            match id {
                Some(id) => create.with_id(id).into(),
                None => create.into(),
            }
        },
        "artifact_group",
        &[
            ("evidence", "artifactElement"),
            ("shared", "artifactElement"),
        ],
        &["case", "p", "t1", "g1", "c1"],
    );
}

#[test]
fn sacm23_art_011_create_artifact_group_sets_its_members_in_order_and_they_stay_put() {
    let mut doc = built();
    let all = [
        "t1", "pa1", "pr1", "g1", "evidence", "shared", "case", "c1", "p", "ag1",
    ];
    let id = create(
        ART_011,
        &mut doc,
        CreateArtifactGroup::new("evidence", named("All")).with_group_members(all),
    );
    assert_eq!(
        (
            id.as_str(),
            strs(element(ART_011, &doc, "artifact_group_1").group_members())
        ),
        ("artifact_group_1", all.to_vec()),
        "{ART_011}: every ArtifactElement kind as a member"
    );
    assert_eq!(
        element(ART_011, &doc, "evidence")
            .children()
            .map(|child| child.id().as_str().to_owned())
            .collect::<Vec<_>>(),
        ["t1", "pa1", "g1", "artifact_group_1"],
        "{ART_011}: a member moved, or the group is not the last child"
    );
    let id = create(
        ART_011,
        &mut doc,
        CreateArtifactGroup::new("shared", named("Repeats")).with_group_members(["t1", "t1"]),
    );
    assert_eq!(
        strs(element(ART_011, &doc, id.as_str()).group_members()),
        ["t1", "t1"],
        "{ART_011}: a repeated member is not kept"
    );
    assert_eq!(
        validation::validate(&doc)
            .iter()
            .map(Diagnostic::code)
            .collect::<Vec<_>>(),
        [Code::MULTIPLICITY_VIOLATION],
        "{ART_011}: the built document has findings other than the repeat's warning"
    );
    let text = write_strict(ART_011, &doc);
    assert!(
        line_with(ART_011, &text, "artifact_group_1")
            .ends_with(&format!(" artifactElement=\"{}\">", all.join(" "))),
        "{ART_011}: the members are not written as given\n{text}"
    );
    let back = read_back(ART_011, &text);
    assert!(back == doc, "{ART_011}: the built document reads back !=");
}

#[test]
fn sacm23_art_011_create_artifact_group_checks_members_after_the_parent_and_id_in_order() {
    let mut doc = built();
    let group = |members: &[&str]| {
        CreateArtifactGroup::new("evidence", named("G")).with_group_members(members.iter().copied())
    };
    // The parent and id checks come before any member check.
    for (what, operation, expected) in [
        (
            "a missing parent",
            CreateArtifactGroup::new("nowhere", named("G")).with_group_members(["nope"]),
            "ParentNotFound",
        ),
        (
            "an illegal parent",
            CreateArtifactGroup::new("p", named("G")).with_group_members(["nope"]),
            "InvalidParent",
        ),
        ("an invalid id", group(&["nope"]).with_id("1g"), "InvalidId"),
        (
            "a duplicate id",
            group(&["nope"]).with_id("t1"),
            "DuplicateId",
        ),
    ] {
        let error = refuse(ART_011, &mut doc, operation);
        assert_eq!(
            refusal(&error).0,
            expected,
            "{ART_011}: {what} with an unknown member"
        );
    }
    // A member that names no element.
    let error = refuse(ART_011, &mut doc, group(&["t1", "nope"]));
    let EditError::ReferenceNotFound { end, id, .. } = &error else {
        panic!("{ART_011}: an unknown member: not ReferenceNotFound: {error:?}");
    };
    assert_eq!(
        (*end, id.as_str(), error.code()),
        (END, "nope", Code::CMD_TARGET_NOT_FOUND),
        "{ART_011}: an unknown member"
    );
    // A UtilityElement member: WrongReferenceType, expected ArtifactElement.
    let error = refuse(ART_011, &mut doc, group(&["t1", "d1"]));
    let EditError::WrongReferenceType {
        end,
        id,
        kind,
        expected,
        ..
    } = &error
    else {
        panic!("{ART_011}: a Description member: not WrongReferenceType: {error:?}");
    };
    assert_eq!(
        (*end, id.as_str(), *kind, *expected, error.code()),
        (
            END,
            "d1",
            ElementKind::Description,
            AbstractClass::ArtifactElement.name(),
            Code::REF_WRONG_TYPE
        ),
        "{ART_011}: a Description member"
    );
    assert_eq!(*expected, "ArtifactElement", "{ART_011}: the expected type");
    // Existence before type, over every member.
    let error = refuse(ART_011, &mut doc, group(&["d1", "nope"]));
    assert!(
        matches!(&error, EditError::ReferenceNotFound { id, .. } if id.as_str() == "nope"),
        "{ART_011}: the missing member is not reported before the wrong type: {error:?}"
    );
    // White space in a member id before everything else about the members.
    let mut doc = match xmi::read(
        "<sacm:ArtifactPackage xmlns:sacm=\"http://www.omg.org/spec/SACM/20220301\" \
         xmlns:xmi=\"http://www.omg.org/spec/XMI/20131001\" xmi:id=\"evidence\">\
         <name content=\"E\"/>\
         <artifactElement xmi:type=\"sacm:Technique\" xmi:id=\"t1\"><name content=\"T\"/>\
         <description xmi:id=\"d1\"><content><value content=\"D\"/></content></description>\
         </artifactElement>\
         <artifactElement xmi:type=\"sacm:Technique\" xmi:id=\"a b\"><name content=\"S\"/>\
         </artifactElement></sacm:ArtifactPackage>"
            .as_bytes(),
        &ReadOptions::default().with_mode(Mode::Tolerant),
    ) {
        Ok(output) => output.into_document(),
        Err(error) => panic!("{ART_011}: setup: a Technique with the id \"a b\": {error}"),
    };
    for given in [
        vec!["a b"],
        vec!["t1", "a b"],
        vec!["nope", "a b"],
        vec!["d1", "a b"],
    ] {
        let error = refuse(ART_011, &mut doc, group(&given));
        let EditError::InvalidReference { end, id, .. } = &error else {
            panic!("{ART_011}: members {given:?}: not InvalidReference: {error:?}");
        };
        assert_eq!(
            (*end, id.as_str(), error.code()),
            (END, "a b", Code::ID_INVALID),
            "{ART_011}: members {given:?}"
        );
    }
}

// =====================================================================================
// SACM23-ART-011 item 3: the reference edits with ArtifactGroupMember
// =====================================================================================

#[test]
fn sacm23_art_011_artifact_group_member_is_the_end_artifact_element() {
    assert_eq!(
        ReferenceEnd::ArtifactGroupMember.name(),
        END,
        "{ART_011}: ArtifactGroupMember.name()"
    );
    assert_ne!(
        ReferenceEnd::ArtifactGroupMember,
        ReferenceEnd::GroupMember,
        "{ART_011}: the two group ends are one variant"
    );
}

#[test]
fn sacm23_art_011_set_add_and_remove_reference_edit_an_artifact_group_s_members() {
    let mut doc = built();
    let mutation = applied(
        ART_011,
        &mut doc,
        SetReferences::new("g1", ReferenceEnd::ArtifactGroupMember, ["t1", "pa1", "t1"]),
    );
    assert_eq!(
        (
            mutation.changed_ids(),
            strs(element(ART_011, &doc, "g1").group_members())
        ),
        (list(&["g1"]).as_slice(), vec!["t1", "pa1", "t1"]),
        "{ART_011}: SetReferences"
    );
    applied(
        ART_011,
        &mut doc,
        AddReference::new("g1", ReferenceEnd::ArtifactGroupMember, "pr1"),
    );
    assert_eq!(
        strs(element(ART_011, &doc, "g1").group_members()),
        ["t1", "pa1", "t1", "pr1"],
        "{ART_011}: AddReference appends"
    );
    applied(
        ART_011,
        &mut doc,
        RemoveReference::new("g1", ReferenceEnd::ArtifactGroupMember, "t1"),
    );
    assert_eq!(
        strs(element(ART_011, &doc, "g1").group_members()),
        ["pa1", "pr1"],
        "{ART_011}: RemoveReference removes every occurrence"
    );
    // The members' own places do not change.
    assert_eq!(
        element(ART_011, &doc, "pr1")
            .parent()
            .map(|parent| parent.id().as_str().to_owned()),
        Some("t1".to_owned()),
        "{ART_011}: a member moved"
    );
    // Refusals: a missing id, a UtilityElement, an id not held.
    let error = refuse(
        ART_011,
        &mut doc,
        AddReference::new("g1", ReferenceEnd::ArtifactGroupMember, "nope"),
    );
    assert!(
        matches!(&error, EditError::ReferenceNotFound { end, id, .. }
            if *end == END && id.as_str() == "nope"),
        "{ART_011}: AddReference of a missing id: {error:?}"
    );
    let error = refuse(
        ART_011,
        &mut doc,
        SetReferences::new("g1", ReferenceEnd::ArtifactGroupMember, ["t1", "d1"]),
    );
    assert!(
        matches!(&error, EditError::WrongReferenceType { end, id, expected, .. }
            if *end == END && id.as_str() == "d1" && *expected == "ArtifactElement"),
        "{ART_011}: SetReferences with a Description: {error:?}"
    );
    let error = refuse(
        ART_011,
        &mut doc,
        RemoveReference::new("g1", ReferenceEnd::ArtifactGroupMember, "c1"),
    );
    assert!(
        matches!(&error, EditError::ReferenceNotHeld { end, id, .. }
            if *end == END && id.as_str() == "c1"),
        "{ART_011}: RemoveReference of an id not held: {error:?}"
    );
    // No bound: clearing leaves the group, written without the end.
    applied(
        ART_011,
        &mut doc,
        SetReferences::clear("g1", ReferenceEnd::ArtifactGroupMember),
    );
    assert!(
        element(ART_011, &doc, "g1").group_members().is_empty(),
        "{ART_011}: clear"
    );
    let text = write_strict(ART_011, &doc);
    assert_eq!(
        line_with(ART_011, &text, "g1"),
        "<artifactElement xmi:type=\"sacm:ArtifactGroup\" xmi:id=\"g1\">",
        "{ART_011}: the cleared end is written\n{text}"
    );
}

#[test]
fn sacm23_art_011_each_group_end_on_the_other_group_kind_is_feature_not_applicable() {
    let mut doc = built();
    for (what, operation, element_id, kind, feature) in [
        (
            "ArtifactGroupMember on an ArgumentGroup",
            Operation::from(SetReferences::new(
                "ag1",
                ReferenceEnd::ArtifactGroupMember,
                ["c1"],
            )),
            "ag1",
            ElementKind::ArgumentGroup,
            END,
        ),
        (
            "AddReference of ArtifactGroupMember on an ArgumentGroup",
            AddReference::new("ag1", ReferenceEnd::ArtifactGroupMember, "c1").into(),
            "ag1",
            ElementKind::ArgumentGroup,
            END,
        ),
        (
            "ArtifactGroupMember on a Technique",
            SetReferences::new("t1", ReferenceEnd::ArtifactGroupMember, ["pa1"]).into(),
            "t1",
            ElementKind::Technique,
            END,
        ),
        (
            "ArtifactGroupMember on an ArtifactPackage",
            SetReferences::new("evidence", ReferenceEnd::ArtifactGroupMember, ["pa1"]).into(),
            "evidence",
            ElementKind::ArtifactPackage,
            END,
        ),
        (
            "GroupMember on an ArtifactGroup",
            SetReferences::new("g1", ReferenceEnd::GroupMember, ["t1"]).into(),
            "g1",
            ElementKind::ArtifactGroup,
            "argumentElement",
        ),
        (
            "RemoveReference of GroupMember on an ArtifactGroup",
            RemoveReference::new("g1", ReferenceEnd::GroupMember, "t1").into(),
            "g1",
            ElementKind::ArtifactGroup,
            "argumentElement",
        ),
    ] {
        let error = refuse(ART_011, &mut doc, operation);
        assert!(
            matches!(
                &error,
                EditError::FeatureNotApplicable { element, kind: k, feature: f, .. }
                    if element.as_str() == element_id && *k == kind && *f == feature
            ),
            "{ART_011}: {what}: not FeatureNotApplicable {{ {element_id}, {kind}, {feature} }}: \
             {error:?}"
        );
        assert_eq!(
            error.code(),
            Code::CMD_FEATURE_NOT_APPLICABLE,
            "{ART_011}: {what}: not SACM-CMD-008"
        );
    }
}

// =====================================================================================
// SACM23-ART-011 item 5: Difference::ArtifactGroupMembers
// =====================================================================================

#[test]
fn sacm23_art_011_semantic_compare_reports_one_artifact_group_members_difference() {
    let with = |members: &[&str]| {
        let mut doc = built();
        applied(
            ART_011,
            &mut doc,
            SetReferences::new(
                "g1",
                ReferenceEnd::ArtifactGroupMember,
                members.iter().copied(),
            ),
        );
        doc
    };
    for (left, right) in [
        (vec!["t1", "pa1"], vec!["pa1", "t1"]),
        (vec!["t1"], vec!["t1", "t1"]),
        (vec!["t1"], Vec::new()),
    ] {
        let found = semantic_compare(&with(&left), &with(&right));
        assert_eq!(
            found,
            [Difference::ArtifactGroupMembers {
                id: ElementId::from("g1"),
                left: list(&left),
                right: list(&right),
            }],
            "{ART_011}: {left:?} vs {right:?}: not one ArtifactGroupMembers difference"
        );
        assert_eq!(
            found[0].element_id(),
            Some(&ElementId::from("g1")),
            "{ART_011}: element_id() does not name the group"
        );
    }
    // After AbstractForm, and apart from an ArgumentGroup's GroupMembers.
    let mut left = with(&["t1"]);
    applied(
        ART_011,
        &mut left,
        SetReferences::new("g1", ReferenceEnd::AbstractForm, ["g1"]),
    );
    applied(
        ART_011,
        &mut left,
        SetReferences::new("ag1", ReferenceEnd::GroupMember, ["c1"]),
    );
    let found = semantic_compare(&left, &built());
    assert!(
        matches!(
            found.as_slice(),
            [
                Difference::GroupMembers { id: ag, .. },
                Difference::AbstractForm { id: g_a, .. },
                Difference::ArtifactGroupMembers { id: g_m, .. }
            ] if ag.as_str() == "ag1" && g_a.as_str() == "g1" && g_m.as_str() == "g1"
        ),
        "{ART_011}: not GroupMembers of ag1, then AbstractForm and ArtifactGroupMembers of g1: \
         {found:?}"
    );
}

#[test]
fn sacm23_art_011_an_artifact_group_members_difference_displays_its_end_and_both_lists() {
    let text = std::fs::read_to_string(FIXTURE)
        .unwrap_or_else(|error| panic!("{ART_011}: cannot read {FIXTURE}: {error}"));
    let read = |text: &str| match xmi::read(text.as_bytes(), &ReadOptions::default()) {
        Ok(output) => output.into_document(),
        Err(error) => panic!("{ART_011}: the fixture does not read: {error}"),
    };
    let left = read(&text.replacen("        <artifactElement href=\"#assessor\"/>\n", "", 1));
    let right = read(&text.replacen(
        "        <artifactElement xmi:idref=\"fta\"/>\n        <artifactElement href=\"#assessor\"/>\n",
        "",
        1,
    ));
    let found = semantic_compare(&left, &right);
    let [difference] = found.as_slice() else {
        panic!("{ART_011}: not one difference: {found:?}");
    };
    assert_eq!(
        difference.to_string(),
        "element \"analysis\" differs in artifactElement: [\"fta\"] on the left, [] on the right",
        "{ART_011}: the Display of ArtifactGroupMembers"
    );
}

// =====================================================================================
// The usage example of the design brief, and the static assertions
// =====================================================================================

#[test]
fn sacm23_art_011_the_usage_example_builds_a_group_of_a_technique_and_a_participant() {
    let mut doc = Document::new();
    let en = |text: &str| LangString::new(text).with_lang("en");
    applied(
        ART_011,
        &mut doc,
        CreateArtifactPackage::root(en("Evidence")).with_id("evidence"),
    );
    applied(
        ART_011,
        &mut doc,
        CreateTechnique::new("evidence", en("Fault tree analysis")).with_id("fta"),
    );
    applied(
        ART_011,
        &mut doc,
        CreateProperty::new("fta", en("Qualified tool")),
    );
    applied(
        ART_011,
        &mut doc,
        CreateParticipant::new("evidence", en("Independent assessor")).with_id("assessor"),
    );
    let group = CreateArtifactGroup::new("evidence", en("Analysis"))
        .with_group_members(["fta", "assessor"]);
    let created = applied(ART_011, &mut doc, group);
    let analysis = element(ART_011, &doc, created.created_ids()[0].as_str());
    assert_eq!(
        (
            analysis.group_members().len(),
            element(ART_011, &doc, "fta").properties().count()
        ),
        (2, 1),
        "{ART_011}: the usage example"
    );
    let xml = match xmi::write(&doc, &WriteOptions::default()) {
        Ok(xml) => xml,
        Err(error) => panic!("{ART_011}: the usage example does not write: {error}"),
    };
    assert!(
        xml.contains(r#"artifactElement="fta assessor""#),
        "{ART_011}: the usage example's output\n{xml}"
    );
    assert_eq!(
        (
            element(ART_011, &doc, "property_1").kind(),
            created.created_ids()[0].as_str()
        ),
        (ElementKind::Property, "artifact_group_1"),
        "{ART_011}: the usage example's generated ids"
    );
}

fn assert_send_sync<T: Send + Sync + Clone + std::fmt::Debug + PartialEq + Eq>() {}

#[test]
fn sacm23_art_001_the_artifact_creates_are_send_sync_clone_debug_and_eq() {
    assert_send_sync::<CreateArtifactPackage>();
    assert_send_sync::<CreateProperty>();
    assert_send_sync::<CreateTechnique>();
    assert_send_sync::<CreateParticipant>();
    assert_send_sync::<CreateArtifactGroup>();
}

#[test]
fn sacm23_art_001_each_artifact_create_converts_into_its_operation_variant() {
    let cases: [(Operation, &str); 5] = [
        (
            CreateArtifactPackage::root(named("A")).into(),
            "CreateArtifactPackage",
        ),
        (
            CreateProperty::new("p", named("A")).into(),
            "CreateProperty",
        ),
        (
            CreateTechnique::new("p", named("A")).into(),
            "CreateTechnique",
        ),
        (
            CreateParticipant::new("p", named("A")).into(),
            "CreateParticipant",
        ),
        (
            CreateArtifactGroup::new("p", named("A")).into(),
            "CreateArtifactGroup",
        ),
    ];
    for (operation, name) in &cases {
        assert_eq!(
            operation.operation_name(),
            *name,
            "{ART_001}: operation_name() of {operation:?}"
        );
    }
    assert!(
        matches!(&cases[0].0, Operation::CreateArtifactPackage(_))
            && matches!(&cases[1].0, Operation::CreateProperty(_))
            && matches!(&cases[2].0, Operation::CreateTechnique(_))
            && matches!(&cases[3].0, Operation::CreateParticipant(_))
            && matches!(&cases[4].0, Operation::CreateArtifactGroup(_)),
        "{ART_001}: the From impls do not give the operation variants"
    );
}
