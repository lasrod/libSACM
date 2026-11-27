//! Tests for LIB-EDIT-025: an edit refuses text XML 1.0 cannot represent.
//!
//! XML Schema Part 2 §3.2.1: the value space of `xsd:string` is the set of
//! sequences of characters that match the `Char` production of XML 1.0; XMI
//! 2.5.1 §8.2.1 rule 4j maps every string-valued property to an attribute of
//! type `xsd:string`. So a string holding a character outside `Char` is no
//! `xsd:string`, no conforming file carries it, and no edit stores it: every
//! operation that stores text refuses it, last, as
//! `EditError::UnrepresentableCharacter` (SACM-XML-003), naming the text as
//! `WriteError::UnrepresentableCharacter` does.
//!
//! - Every text an operation stores is checked (the brief's §2 table): names,
//!   gids, versions, dates, startTimes, endTimes, values, externalReferences,
//!   and the values of a content, a key and a location with their position.
//! - The first character of the first text is named, in the writer's order.
//! - Every other refusal comes first; `check`, `preview`, `apply` and the
//!   batches agree, and a refusal changes nothing.
//! - An operation that stores nothing is not checked.
//! - A read cannot hold such text, and a document built by accepted edits
//!   always writes.
//!
//! `edit::apply_unchecked_text`, the test-support seam, builds the documents
//! that hold such text, so that the write refusal stays reachable.

use std::collections::BTreeMap;
use std::path::Path;

use sacm::diagnostic::Code;
use sacm::edit::{
    self, AddReference, BatchError, CopyElements, CreateActivity, CreateArgumentGroup,
    CreateArgumentPackage, CreateArgumentPackageBinding, CreateArgumentPackageInterface,
    CreateArgumentReasoning, CreateArtifact, CreateArtifactAssetRelationship, CreateArtifactGroup,
    CreateArtifactPackage, CreateArtifactPackageBinding, CreateArtifactPackageInterface,
    CreateArtifactReference, CreateAssertedArtifactContext, CreateAssertedArtifactSupport,
    CreateAssertedContext, CreateAssertedEvidence, CreateAssertedInference,
    CreateAssuranceCasePackage, CreateAssuranceCasePackageBinding,
    CreateAssuranceCasePackageInterface, CreateCategory, CreateClaim, CreateDescription,
    CreateEvent, CreateExpression, CreateImplementationConstraint, CreateNote, CreateParticipant,
    CreateProperty, CreateResource, CreateTaggedValue, CreateTechnique, CreateTerm,
    CreateTerminologyGroup, CreateTerminologyPackage, CreateTerminologyPackageBinding,
    CreateTerminologyPackageInterface, CrossPackageReferencePolicy, DeleteElement, EditError,
    MoveElement, Mutation, Operation, PackageDeletePolicy, ReferenceDeletePolicy, ReorderElements,
    SetAssertionDeclaration, SetContent, SetDate, SetEndTime, SetExternalReference, SetFlag,
    SetGid, SetKey, SetLocation, SetName, SetStartTime, SetTaggedValue, SetValue, SetVersion,
};
use sacm::model::{
    AssertionDeclaration, Document, ElementId, ElementKind, Flag, LangString, MultiLangString,
    ReferenceEnd,
};
use sacm::xmi::{self, Mode, ReadError, ReadOptions, WriteAttribute, WriteError, WriteOptions};

const ROW: &str = "LIB-EDIT-025";

const MODES: [Mode; 2] = [Mode::Strict, Mode::Tolerant];

/// An id no element of the start document has.
const NOWHERE: &str = "nowhere";

/// The caller id given to the creates.
const GIVEN: &str = "new_one";

/// The namespace declarations of a root: `sacm` and `xmi`.
const NS: &str = concat!(
    " xmlns:sacm=\"http://www.omg.org/spec/SACM/20220301\"",
    " xmlns:xmi=\"http://www.omg.org/spec/XMI/20131001\"",
);

/// The catalog of diagnostic codes, relative to the repository root.
const CATALOG: &str = "docs/sacm/diagnostics.md";

/// What an `UnrepresentableCharacter` refusal names: the element, the
/// character, the attribute and the position.
type Bad = (String, char, WriteAttribute, Option<usize>);

// ---------------------------------------------------------------------------
// The designed API of LIB-EDIT-025: the new variant and the test seam
// ---------------------------------------------------------------------------

/// The element, character, attribute and position of an
/// `EditError::UnrepresentableCharacter`; `None` for any other refusal.
fn unrepresentable(error: &EditError) -> Option<Bad> {
    match error {
        EditError::UnrepresentableCharacter {
            element,
            character,
            attribute,
            position,
            ..
        } => Some((
            element.as_str().to_owned(),
            *character,
            *attribute,
            *position,
        )),
        _ => None,
    }
}

/// Applies `operation` with the test seam, which skips the text check, so
/// that `doc` can hold text XML 1.0 cannot represent.
fn seam(doc: &mut Document, operation: impl Into<Operation>) -> Mutation {
    let operation = operation.into();
    match edit::apply_unchecked_text(doc, operation.clone()) {
        Ok(mutation) => mutation,
        Err(error) => panic!("{ROW}: setup: the seam refuses {operation:?}: {error}"),
    }
}

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

fn bad(element: &str, character: char, attribute: WriteAttribute, position: Option<usize>) -> Bad {
    (element.to_owned(), character, attribute, position)
}

fn named(content: &str) -> LangString {
    LangString::new(content).with_lang("en")
}

/// The values `values`, in order.
fn multi(values: &[LangString]) -> MultiLangString {
    values.iter().cloned().collect()
}

/// Applies `operation`, which the test expects to be accepted, after
/// checking that `check` accepts it and `preview` gives `apply`'s mutation.
fn accepted(doc: &mut Document, operation: impl Into<Operation>) -> Mutation {
    let operation = operation.into();
    let checked = edit::check(doc, &operation);
    let previewed = edit::preview(doc, &operation).map(|preview| preview.mutation().clone());
    let mutation = match edit::apply(doc, operation.clone()) {
        Ok(mutation) => mutation,
        Err(error) => panic!("{ROW}: {operation:?} was refused: {error} ({error:?})"),
    };
    assert_eq!(
        (checked, previewed.as_ref()),
        (Ok(()), Ok(&mutation)),
        "{ROW}: check or preview answer {operation:?} otherwise than apply"
    );
    mutation
}

/// Applies `operation`, which the test expects to be refused for a
/// character XML 1.0 cannot represent, and returns what the refusal names.
/// The document is `==` to before with the same revision, `check` and
/// `preview` refuse it alike, the code is SACM-XML-003 and the message is
/// one line in lower case.
fn refused(doc: &mut Document, operation: impl Into<Operation>) -> Bad {
    let operation = operation.into();
    let error = refusal(doc, &operation);
    assert_eq!(
        error.code(),
        Code::XML_UNREPRESENTABLE_CHARACTER,
        "{ROW}: the code of the refusal of {operation:?}"
    );
    let shown = error.to_string();
    assert!(
        !shown.contains('\n') && shown.starts_with(char::is_lowercase),
        "{ROW}: the message {shown:?} is not one line starting in lower case"
    );
    unrepresentable(&error).unwrap_or_else(|| {
        panic!("{ROW}: {operation:?} is not refused as UnrepresentableCharacter: {error:?}")
    })
}

/// Applies `operation`, which the test expects to be refused, and returns
/// the refusal, after checking that it changed nothing and that `check` and
/// `preview` refuse it alike.
fn refusal(doc: &mut Document, operation: &Operation) -> EditError {
    let before = doc.clone();
    let revision = doc.revision();
    let checked = edit::check(doc, operation);
    let previewed = edit::preview(doc, operation).map(|preview| preview.mutation().clone());
    let error = match edit::apply(doc, operation.clone()) {
        Ok(mutation) => panic!("{ROW}: {operation:?} was accepted: {mutation:?}"),
        Err(error) => error,
    };
    assert!(
        *doc == before && doc.revision() == revision,
        "{ROW}: the refused {operation:?} changed the document"
    );
    assert_eq!(
        (checked, previewed),
        (Err(error.clone()), Err(error.clone())),
        "{ROW}: check or preview answer {operation:?} otherwise than apply"
    );
    error
}

/// The id the create `operation` would generate on `doc`: the id it creates
/// on a clone.
fn generated(doc: &Document, operation: Operation) -> String {
    let mut probe = doc.clone();
    let mutation = accepted(&mut probe, operation);
    match mutation.created_id() {
        Some(id) => id.as_str().to_owned(),
        None => panic!("{ROW}: setup: the probe created no element: {mutation:?}"),
    }
}

/// The document every test edits, built by edits:
/// - the `AssuranceCasePackage` root `case`, holding the `ArgumentPackage`s `p`
///   (the Claims `c1` and `c2`) and `q`, the `ArtifactPackage`s `a` (the
///   Artifact `report`, the Event `ev1`, the Activity `act1` and the Resource
///   `r1`) and `a2`, the `AssuranceCasePackage` `case2`, and the
///   `TerminologyPackage`s `terms` (the Term `t1` and the Expression `e1`) and
///   `terms2`;
/// - `c1` holds the Note `n1` and the `TaggedValue` `tv1`, with the key `k`.
fn start() -> Document {
    let mut doc = Document::new();
    let operations: Vec<Operation> = vec![
        CreateAssuranceCasePackage::root(named("Case"))
            .with_id("case")
            .into(),
        CreateArgumentPackage::new("case", named("P"))
            .with_id("p")
            .into(),
        CreateClaim::new("p", named("C1")).with_id("c1").into(),
        CreateClaim::new("p", named("C2")).with_id("c2").into(),
        CreateArgumentPackage::new("case", named("Q"))
            .with_id("q")
            .into(),
        CreateArtifactPackage::new("case", named("A"))
            .with_id("a")
            .into(),
        CreateArtifact::new("a", named("Report"))
            .with_id("report")
            .into(),
        CreateEvent::new("a", named("Released"))
            .with_id("ev1")
            .into(),
        CreateActivity::new("a", named("Testing"))
            .with_id("act1")
            .into(),
        CreateResource::new("a", named("Report PDF"))
            .with_id("r1")
            .into(),
        CreateArtifactPackage::new("case", named("A2"))
            .with_id("a2")
            .into(),
        CreateAssuranceCasePackage::new("case", named("Case 2"))
            .with_id("case2")
            .into(),
        CreateTerminologyPackage::new("case", named("Terms"))
            .with_id("terms")
            .into(),
        CreateTerm::new("terms", named("Blade"), "blade")
            .with_id("t1")
            .into(),
        CreateExpression::new("terms", named("Guarded"), "$Blade$ is guarded")
            .with_id("e1")
            .into(),
        CreateTerminologyPackage::new("case", named("Terms 2"))
            .with_id("terms2")
            .into(),
        CreateNote::new("c1", named("N")).with_id("n1").into(),
        CreateTaggedValue::new("c1", LangString::new("k"), named("v"))
            .with_id("tv1")
            .into(),
    ];
    for operation in operations {
        accepted(&mut doc, operation);
    }
    doc
}

/// `operation` with the caller id `id`, if given.
macro_rules! with_caller {
    ($create:expr, $id:expr) => {
        match $id {
            Some(id) => Operation::from($create.with_id(id)),
            None => Operation::from($create),
        }
    };
}

/// One create of each of the 34 `ModelElement` kinds the edit API creates,
/// named `name`, under a parent of [`start`] that admits it, with the
/// caller id `id` or a generated one.
fn named_creates(name: &LangString, id: Option<&str>) -> Vec<Operation> {
    let n = || name.clone();
    vec![
        with_caller!(CreateAssuranceCasePackage::new("case", n()), id),
        with_caller!(
            CreateAssuranceCasePackageInterface::new("case", n(), "case2"),
            id
        ),
        with_caller!(
            CreateAssuranceCasePackageBinding::new("case", n(), ["case", "case2"]),
            id
        ),
        with_caller!(CreateArgumentPackage::new("case", n()), id),
        with_caller!(CreateArgumentPackageInterface::new("case", n(), "p"), id),
        with_caller!(
            CreateArgumentPackageBinding::new("case", n(), ["p", "q"]),
            id
        ),
        with_caller!(CreateClaim::new("p", n()), id),
        with_caller!(CreateArgumentReasoning::new("p", n()), id),
        with_caller!(CreateAssertedInference::new("p", n(), ["c1"], "c2"), id),
        with_caller!(CreateAssertedContext::new("p", n(), ["c1"], "c2"), id),
        with_caller!(CreateArtifactReference::new("p", n()), id),
        with_caller!(CreateArgumentGroup::new("p", n()), id),
        with_caller!(CreateAssertedEvidence::new("p", n(), ["c1"], "c2"), id),
        with_caller!(
            CreateAssertedArtifactSupport::new("p", n(), ["c1"], "c2"),
            id
        ),
        with_caller!(
            CreateAssertedArtifactContext::new("p", n(), ["c1"], "c2"),
            id
        ),
        with_caller!(CreateArtifactPackage::new("case", n()), id),
        with_caller!(CreateArtifactPackageInterface::new("case", n(), "a"), id),
        with_caller!(
            CreateArtifactPackageBinding::new("case", n(), ["a", "a2"]),
            id
        ),
        with_caller!(CreateProperty::new("a", n()), id),
        with_caller!(CreateTechnique::new("a", n()), id),
        with_caller!(CreateParticipant::new("a", n()), id),
        with_caller!(CreateArtifactGroup::new("a", n()), id),
        with_caller!(CreateArtifact::new("a", n()), id),
        with_caller!(CreateEvent::new("a", n()), id),
        with_caller!(CreateResource::new("a", n()), id),
        with_caller!(CreateActivity::new("a", n()), id),
        with_caller!(
            CreateArtifactAssetRelationship::new("a", n(), ["report"], ["report"]),
            id
        ),
        with_caller!(CreateTerminologyPackage::new("case", n()), id),
        with_caller!(
            CreateTerminologyPackageInterface::new("case", n(), "terms"),
            id
        ),
        with_caller!(
            CreateTerminologyPackageBinding::new("case", n(), ["terms", "terms2"]),
            id
        ),
        with_caller!(CreateCategory::new("terms", n()), id),
        with_caller!(CreateTerm::new("terms", n(), "blade"), id),
        with_caller!(CreateExpression::new("terms", n(), "x"), id),
        with_caller!(CreateTerminologyGroup::new("terms", n()), id),
    ]
}

/// The root creates: the four package kinds, and an interface and a binding
/// of `ArgumentPackage`s, named `name`, with the caller id `id` or a
/// generated one.
fn root_creates(name: &LangString, id: Option<&str>) -> Vec<Operation> {
    let n = || name.clone();
    vec![
        with_caller!(CreateAssuranceCasePackage::root(n()), id),
        with_caller!(CreateArgumentPackage::root(n()), id),
        with_caller!(CreateArtifactPackage::root(n()), id),
        with_caller!(CreateTerminologyPackage::root(n()), id),
        with_caller!(CreateArgumentPackageInterface::root(n(), "p"), id),
        with_caller!(CreateArgumentPackageBinding::root(n(), ["p", "q"]), id),
    ]
}

/// What both modes of `xmi::write` refuse `doc` with, which must be an
/// `UnrepresentableCharacter`, the same in both.
fn write_refusal(doc: &Document) -> Bad {
    let mut found = Vec::new();
    for mode in MODES {
        match xmi::write(doc, &WriteOptions::default().with_mode(mode)) {
            Ok(text) => panic!("{ROW}: a {mode:?} write writes {text}"),
            Err(WriteError::UnrepresentableCharacter {
                element,
                character,
                attribute,
                position,
                ..
            }) => found.push((element.as_str().to_owned(), character, attribute, position)),
            Err(other) => panic!("{ROW}: a {mode:?} write refuses otherwise: {other:?}"),
        }
    }
    assert_eq!(found[0], found[1], "{ROW}: the modes refuse otherwise");
    found.swap_remove(0)
}

/// `doc` written in `mode`, which must succeed.
fn written(what: &str, doc: &Document, mode: Mode) -> String {
    match xmi::write(doc, &WriteOptions::default().with_mode(mode)) {
        Ok(text) => text,
        Err(error) => panic!("{ROW}: {what}: a {mode:?} write refuses: {error} ({error:?})"),
    }
}

/// `text` read tolerantly, which must succeed.
fn read_back(what: &str, text: &str) -> Document {
    match xmi::read(
        text.as_bytes(),
        &ReadOptions::default().with_mode(Mode::Tolerant),
    ) {
        Ok(output) => output.into_document(),
        Err(error) => panic!("{ROW}: {what}: the output does not read back: {error}\n{text}"),
    }
}

/// The ids the next generated Claim, Note, `TaggedValue`, Artifact and Term
/// of `doc` would get.
fn next_ids(doc: &Document) -> Vec<String> {
    [
        Operation::from(CreateClaim::new("p", named("x"))),
        CreateNote::new("c1", named("x")).into(),
        CreateTaggedValue::new("c1", named("k2"), named("x")).into(),
        CreateArtifact::new("a", named("x")).into(),
        CreateTerm::new("terms", named("x"), "v").into(),
    ]
    .into_iter()
    .map(|operation| generated(doc, operation))
    .collect()
}

// ---------------------------------------------------------------------------
// The texts each operation stores
// ---------------------------------------------------------------------------

#[test]
fn lib_edit_025_a_create_refuses_a_name_xml_cannot_represent() {
    let mut doc = start();
    let cases = [
        (
            LangString::new("ok").with_lang("e\u{1}n"),
            '\u{1}',
            WriteAttribute::NameLang,
        ),
        (
            LangString::new("o\u{2}k").with_lang("en"),
            '\u{2}',
            WriteAttribute::NameContent,
        ),
    ];
    let count = named_creates(&named("ok"), None).len();
    assert_eq!(count, 34, "{ROW}: setup: not one create of each named kind");
    let names: std::collections::BTreeSet<&str> = named_creates(&named("ok"), None)
        .iter()
        .map(Operation::operation_name)
        .collect();
    assert_eq!(
        names.len(),
        34,
        "{ROW}: setup: a create kind is listed twice"
    );
    for index in 0..count {
        let probe = named_creates(&named("ok"), None).swap_remove(index);
        let name = probe.operation_name();
        let id = generated(&doc, probe);
        for (bad_name, character, attribute) in &cases {
            let given = named_creates(bad_name, Some(GIVEN)).swap_remove(index);
            assert_eq!(
                refused(&mut doc, given),
                bad(GIVEN, *character, *attribute, None),
                "{ROW}: {name} with the id {GIVEN} and a bad {attribute}"
            );
            let generated_one = named_creates(bad_name, None).swap_remove(index);
            assert_eq!(
                refused(&mut doc, generated_one),
                bad(&id, *character, *attribute, None),
                "{ROW}: {name} with a generated id and a bad {attribute} does not name {id}"
            );
        }
    }
    assert_eq!(
        generated(&doc, CreateClaim::new("p", named("ok")).into()),
        "claim_1",
        "{ROW}: setup: the next generated Claim id"
    );
    let name = LangString::new("o\u{2}k");
    assert_eq!(
        refused(&mut doc, CreateClaim::new("p", name)),
        bad("claim_1", '\u{2}', WriteAttribute::NameContent, None),
        "{ROW}: a Claim with a generated id names claim_1"
    );
    // The root creates too.
    for (index, probe) in root_creates(&named("ok"), None).into_iter().enumerate() {
        let name = probe.operation_name();
        let id = generated(&doc, probe);
        for (bad_name, character, attribute) in &cases {
            let given = root_creates(bad_name, Some(GIVEN)).swap_remove(index);
            assert_eq!(
                refused(&mut doc, given),
                bad(GIVEN, *character, *attribute, None),
                "{ROW}: the root {name} with the id {GIVEN} and a bad {attribute}"
            );
            let generated_one = root_creates(bad_name, None).swap_remove(index);
            assert_eq!(
                refused(&mut doc, generated_one),
                bad(&id, *character, *attribute, None),
                "{ROW}: the root {name} with a generated id and a bad {attribute}"
            );
        }
    }
}

#[test]
fn lib_edit_025_set_name_refuses_a_name_xml_cannot_represent() {
    let mut doc = start();
    for target in ["c1", "p", "case", "report", "t1"] {
        assert_eq!(
            refused(
                &mut doc,
                SetName::new(target, LangString::new("ok").with_lang("\u{1}"))
            ),
            bad(target, '\u{1}', WriteAttribute::NameLang, None),
            "{ROW}: SetName with a bad lang on {target}"
        );
        assert_eq!(
            refused(&mut doc, SetName::new(target, LangString::new("x\u{1F}"))),
            bad(target, '\u{1F}', WriteAttribute::NameContent, None),
            "{ROW}: SetName with a bad content on {target}"
        );
    }
    // A good name is still set.
    accepted(&mut doc, SetName::new("c1", named("Renamed")));
}

#[test]
fn lib_edit_025_set_gid_refuses_a_gid_xml_cannot_represent() {
    let mut doc = start();
    for target in ["p", "c1", "n1", "tv1"] {
        assert_eq!(
            refused(&mut doc, SetGid::new(target, "g\u{1}")),
            bad(target, '\u{1}', WriteAttribute::Gid, None),
            "{ROW}: SetGid on {target}"
        );
    }
    // A remove stores nothing, so a held bad gid is removed.
    seam(&mut doc, SetGid::new("p", "g\u{1}"));
    seam(&mut doc, SetGid::new("n1", "g\u{1}"));
    for target in ["p", "n1"] {
        let mutation = accepted(&mut doc, SetGid::remove(target));
        assert_eq!(
            mutation.changed_ids(),
            [ElementId::from(target)],
            "{ROW}: SetGid::remove on {target} holding a bad gid"
        );
    }
}

#[test]
fn lib_edit_025_a_version_is_refused_on_create_and_set() {
    let mut doc = start();
    assert_eq!(
        refused(
            &mut doc,
            CreateArtifact::new("a", named("A"))
                .with_version("1.\u{1}")
                .with_id(GIVEN)
        ),
        bad(GIVEN, '\u{1}', WriteAttribute::Version, None),
        "{ROW}: CreateArtifact with a bad version"
    );
    assert_eq!(
        refused(&mut doc, SetVersion::new("report", "1.\u{1}")),
        bad("report", '\u{1}', WriteAttribute::Version, None),
        "{ROW}: SetVersion with a bad version"
    );
}

#[test]
fn lib_edit_025_a_date_is_refused_on_create_and_set() {
    let mut doc = start();
    assert_eq!(
        refused(
            &mut doc,
            CreateArtifact::new("a", named("A"))
                .with_date("2024\u{2}")
                .with_id(GIVEN)
        ),
        bad(GIVEN, '\u{2}', WriteAttribute::Date, None),
        "{ROW}: CreateArtifact with a bad date"
    );
    assert_eq!(
        refused(
            &mut doc,
            CreateEvent::new("a", named("E"))
                .with_date("2024\u{2}")
                .with_id(GIVEN)
        ),
        bad(GIVEN, '\u{2}', WriteAttribute::Date, None),
        "{ROW}: CreateEvent with a bad date"
    );
    for target in ["report", "ev1"] {
        assert_eq!(
            refused(&mut doc, SetDate::new(target, "2024\u{2}")),
            bad(target, '\u{2}', WriteAttribute::Date, None),
            "{ROW}: SetDate on {target}"
        );
    }
}

#[test]
fn lib_edit_025_a_start_time_and_an_end_time_are_refused_on_create_and_set() {
    let mut doc = start();
    assert_eq!(
        refused(
            &mut doc,
            CreateActivity::new("a", named("A"))
                .with_start_time("t\u{3}")
                .with_id(GIVEN)
        ),
        bad(GIVEN, '\u{3}', WriteAttribute::StartTime, None),
        "{ROW}: CreateActivity with a bad startTime"
    );
    assert_eq!(
        refused(
            &mut doc,
            CreateActivity::new("a", named("A"))
                .with_end_time("t\u{3}")
                .with_id(GIVEN)
        ),
        bad(GIVEN, '\u{3}', WriteAttribute::EndTime, None),
        "{ROW}: CreateActivity with a bad endTime"
    );
    assert_eq!(
        refused(&mut doc, SetStartTime::new("act1", "t\u{3}")),
        bad("act1", '\u{3}', WriteAttribute::StartTime, None),
        "{ROW}: SetStartTime with a bad startTime"
    );
    assert_eq!(
        refused(&mut doc, SetEndTime::new("act1", "t\u{3}")),
        bad("act1", '\u{3}', WriteAttribute::EndTime, None),
        "{ROW}: SetEndTime with a bad endTime"
    );
}

#[test]
fn lib_edit_025_a_value_is_refused_on_create_and_set() {
    let mut doc = start();
    assert_eq!(
        refused(
            &mut doc,
            CreateTerm::new("terms", named("T"), "v\u{FFFE}").with_id(GIVEN)
        ),
        bad(GIVEN, '\u{FFFE}', WriteAttribute::Value, None),
        "{ROW}: CreateTerm with a bad value"
    );
    assert_eq!(
        refused(
            &mut doc,
            CreateExpression::new("terms", named("E"), "v\u{FFFE}").with_id(GIVEN)
        ),
        bad(GIVEN, '\u{FFFE}', WriteAttribute::Value, None),
        "{ROW}: CreateExpression with a bad value"
    );
    for target in ["t1", "e1"] {
        assert_eq!(
            refused(&mut doc, SetValue::new(target, "v\u{FFFE}")),
            bad(target, '\u{FFFE}', WriteAttribute::Value, None),
            "{ROW}: SetValue on {target}"
        );
    }
}

#[test]
fn lib_edit_025_an_external_reference_is_refused_on_create_and_set() {
    let mut doc = start();
    assert_eq!(
        refused(
            &mut doc,
            CreateTerm::new("terms", named("T"), "v")
                .with_external_reference("u\u{FFFF}")
                .with_id(GIVEN)
        ),
        bad(GIVEN, '\u{FFFF}', WriteAttribute::ExternalReference, None),
        "{ROW}: CreateTerm with a bad externalReference"
    );
    assert_eq!(
        refused(&mut doc, SetExternalReference::new("t1", "u\u{FFFF}")),
        bad("t1", '\u{FFFF}', WriteAttribute::ExternalReference, None),
        "{ROW}: SetExternalReference with a bad externalReference"
    );
}

/// Content values with a bad content at index 1, and with a bad lang at
/// index 0, with what each refusal names on `element`.
fn bad_values(
    lang: WriteAttribute,
    content: WriteAttribute,
) -> [(MultiLangString, char, WriteAttribute, usize); 2] {
    [
        (
            multi(&[LangString::new("ok"), LangString::new("bad\u{4}")]),
            '\u{4}',
            content,
            1,
        ),
        (
            multi(&[LangString::new("ok").with_lang("\u{5}"), named("fine")]),
            '\u{5}',
            lang,
            0,
        ),
    ]
}

#[test]
fn lib_edit_025_content_values_are_refused_with_their_position() {
    let mut doc = start();
    let cases = bad_values(
        WriteAttribute::ContentValueLang,
        WriteAttribute::ContentValueContent,
    );
    for (content, character, attribute, position) in cases {
        let at = Some(position);
        let creates: [Operation; 4] = [
            CreateDescription::new("c1", content.clone())
                .with_id(GIVEN)
                .into(),
            CreateImplementationConstraint::new("c1", content.clone())
                .with_id(GIVEN)
                .into(),
            CreateNote::new("c1", content.clone()).with_id(GIVEN).into(),
            CreateTaggedValue::new("c1", named("key"), content.clone())
                .with_id(GIVEN)
                .into(),
        ];
        for create in creates {
            let name = create.operation_name();
            assert_eq!(
                refused(&mut doc, create),
                bad(GIVEN, character, attribute, at),
                "{ROW}: {name} with a bad {attribute} at {position}"
            );
        }
        for target in ["n1", "tv1"] {
            assert_eq!(
                refused(&mut doc, SetContent::new(target, content.clone())),
                bad(target, character, attribute, at),
                "{ROW}: SetContent on {target} with a bad {attribute} at {position}"
            );
        }
    }
}

#[test]
fn lib_edit_025_key_values_are_refused_with_their_position() {
    let mut doc = start();
    let cases = bad_values(
        WriteAttribute::KeyValueLang,
        WriteAttribute::KeyValueContent,
    );
    for (key, character, attribute, position) in cases {
        let at = Some(position);
        assert_eq!(
            refused(
                &mut doc,
                CreateTaggedValue::new("c1", key.clone(), named("v")).with_id(GIVEN)
            ),
            bad(GIVEN, character, attribute, at),
            "{ROW}: CreateTaggedValue with a bad {attribute} at {position}"
        );
        assert_eq!(
            refused(&mut doc, SetKey::new("tv1", key.clone())),
            bad("tv1", character, attribute, at),
            "{ROW}: SetKey with a bad {attribute} at {position}"
        );
    }
}

#[test]
fn lib_edit_025_location_values_are_refused_with_their_position() {
    let mut doc = start();
    let cases = bad_values(
        WriteAttribute::LocationValueLang,
        WriteAttribute::LocationValueContent,
    );
    for (location, character, attribute, position) in cases {
        let at = Some(position);
        assert_eq!(
            refused(
                &mut doc,
                CreateResource::new("a", named("R"))
                    .with_location(location.clone())
                    .with_id(GIVEN)
            ),
            bad(GIVEN, character, attribute, at),
            "{ROW}: CreateResource with a bad {attribute} at {position}"
        );
        assert_eq!(
            refused(&mut doc, SetLocation::new("r1", location.clone())),
            bad("r1", character, attribute, at),
            "{ROW}: SetLocation with a bad {attribute} at {position}"
        );
    }
}

#[test]
fn lib_edit_025_set_tagged_value_refuses_its_content_then_its_key() {
    let mut doc = start();
    // c2 has no tagged value: SetTaggedValue would create one.
    let created = generated(&doc, SetTaggedValue::new("c2", "k", "ok").into());
    assert_eq!(
        created, "tagged_value_1",
        "{ROW}: setup: the id SetTaggedValue would generate"
    );
    let content = multi(&[LangString::new("ok"), LangString::new("bad\u{4}")]);
    assert_eq!(
        refused(&mut doc, SetTaggedValue::new("c2", "k", content.clone())),
        bad(
            &created,
            '\u{4}',
            WriteAttribute::ContentValueContent,
            Some(1)
        ),
        "{ROW}: SetTaggedValue creating with a bad content value"
    );
    assert_eq!(
        refused(&mut doc, SetTaggedValue::new("c2", "k\u{1}", "ok")),
        bad(&created, '\u{1}', WriteAttribute::KeyValueContent, Some(0)),
        "{ROW}: SetTaggedValue creating with a bad key: the key's content at index 0"
    );
    assert_eq!(
        refused(&mut doc, SetTaggedValue::new("c2", "k\u{1}", "c\u{2}")),
        bad(
            &created,
            '\u{2}',
            WriteAttribute::ContentValueContent,
            Some(0)
        ),
        "{ROW}: SetTaggedValue creating: the content before the key"
    );
    // c1's tv1 has the key k: SetTaggedValue sets its content.
    assert_eq!(
        refused(&mut doc, SetTaggedValue::new("c1", "k", content)),
        bad("tv1", '\u{4}', WriteAttribute::ContentValueContent, Some(1)),
        "{ROW}: SetTaggedValue setting tv1's content"
    );
    assert_eq!(
        refused(
            &mut doc,
            SetTaggedValue::new("c1", "k", LangString::new("v").with_lang("\u{3}"))
        ),
        bad("tv1", '\u{3}', WriteAttribute::ContentValueLang, Some(0)),
        "{ROW}: SetTaggedValue setting tv1's content with a bad lang"
    );
}

// ---------------------------------------------------------------------------
// The order of the texts
// ---------------------------------------------------------------------------

#[test]
fn lib_edit_025_the_first_character_of_the_first_text_is_named() {
    let mut doc = start();
    let name = |text: &str| LangString::new(text);
    // Within a string, the first such character.
    for (text, first) in [
        ("a\u{1}b\u{FFFE}", '\u{1}'),
        ("a\u{FFFE}b\u{1}", '\u{FFFE}'),
    ] {
        assert_eq!(
            refused(&mut doc, SetName::new("c1", name(text))),
            bad("c1", first, WriteAttribute::NameContent, None),
            "{ROW}: the first character of {text:?}"
        );
    }
    // A name's lang before its content.
    assert_eq!(
        refused(
            &mut doc,
            SetName::new("c1", name("c\u{3}").with_lang("e\u{2}"))
        ),
        bad("c1", '\u{2}', WriteAttribute::NameLang, None),
        "{ROW}: the lang before the content"
    );
    // Values by index, and within one value the lang before the content.
    let content = multi(&[name("x\u{5}"), name("y").with_lang("\u{4}")]);
    assert_eq!(
        refused(&mut doc, SetContent::new("n1", content)),
        bad("n1", '\u{5}', WriteAttribute::ContentValueContent, Some(0)),
        "{ROW}: index 0 before index 1"
    );
    let content = multi(&[name("x\u{5}").with_lang("\u{4}")]);
    assert_eq!(
        refused(&mut doc, SetContent::new("n1", content)),
        bad("n1", '\u{4}', WriteAttribute::ContentValueLang, Some(0)),
        "{ROW}: within a value, the lang before the content"
    );
    assert_artifact_order(&mut doc);
    assert_activity_order(&mut doc);
    assert_term_order(&mut doc);
    // CreateResource: the name, then the location.
    let resource = |name_text: &str| {
        CreateResource::new("a", name(name_text))
            .with_location(multi(&[name("l\u{2}")]))
            .with_id(GIVEN)
    };
    assert_eq!(
        refused(&mut doc, resource("n\u{1}")),
        bad(GIVEN, '\u{1}', WriteAttribute::NameContent, None),
        "{ROW}: CreateResource: the name before the location"
    );
    assert_eq!(
        refused(&mut doc, resource("n")),
        bad(
            GIVEN,
            '\u{2}',
            WriteAttribute::LocationValueContent,
            Some(0)
        ),
        "{ROW}: CreateResource: then the location"
    );
    // CreateTaggedValue: the content, then the key.
    let tagged =
        |content: &str| CreateTaggedValue::new("c1", name("k\u{1}"), name(content)).with_id(GIVEN);
    assert_eq!(
        refused(&mut doc, tagged("c\u{2}")),
        bad(GIVEN, '\u{2}', WriteAttribute::ContentValueContent, Some(0)),
        "{ROW}: CreateTaggedValue: the content before the key"
    );
    assert_eq!(
        refused(&mut doc, tagged("c")),
        bad(GIVEN, '\u{1}', WriteAttribute::KeyValueContent, Some(0)),
        "{ROW}: CreateTaggedValue: then the key"
    );
}

/// `CreateArtifact`: the version, then the date, then the name.
fn assert_artifact_order(doc: &mut Document) {
    let artifact = |version: &str, date: &str| {
        CreateArtifact::new("a", LangString::new("n\u{3}"))
            .with_version(version)
            .with_date(date)
            .with_id(GIVEN)
    };
    for (operation, expected, what) in [
        (
            artifact("v\u{1}", "d\u{2}"),
            bad(GIVEN, '\u{1}', WriteAttribute::Version, None),
            "the version first",
        ),
        (
            artifact("v", "d\u{2}"),
            bad(GIVEN, '\u{2}', WriteAttribute::Date, None),
            "then the date",
        ),
        (
            artifact("v", "d"),
            bad(GIVEN, '\u{3}', WriteAttribute::NameContent, None),
            "then the name",
        ),
    ] {
        assert_eq!(
            refused(doc, operation),
            expected,
            "{ROW}: CreateArtifact: {what}"
        );
    }
}

/// `CreateActivity`: the `startTime`, then the `endTime`, then the name.
fn assert_activity_order(doc: &mut Document) {
    let activity = |start: &str, end: &str| {
        CreateActivity::new("a", LangString::new("n").with_lang("\u{3}"))
            .with_start_time(start)
            .with_end_time(end)
            .with_id(GIVEN)
    };
    for (operation, expected, what) in [
        (
            activity("s\u{1}", "e\u{2}"),
            bad(GIVEN, '\u{1}', WriteAttribute::StartTime, None),
            "the startTime first",
        ),
        (
            activity("s", "e\u{2}"),
            bad(GIVEN, '\u{2}', WriteAttribute::EndTime, None),
            "then the endTime",
        ),
        (
            activity("s", "e"),
            bad(GIVEN, '\u{3}', WriteAttribute::NameLang, None),
            "then the name",
        ),
    ] {
        assert_eq!(
            refused(doc, operation),
            expected,
            "{ROW}: CreateActivity: {what}"
        );
    }
}

/// `CreateTerm`: the value, then the `externalReference`, then the name.
fn assert_term_order(doc: &mut Document) {
    let term = |value: &str, reference: &str| {
        CreateTerm::new("terms", LangString::new("n\u{3}"), value)
            .with_external_reference(reference)
            .with_id(GIVEN)
    };
    for (operation, expected, what) in [
        (
            term("v\u{1}", "u\u{2}"),
            bad(GIVEN, '\u{1}', WriteAttribute::Value, None),
            "the value first",
        ),
        (
            term("v", "u\u{2}"),
            bad(GIVEN, '\u{2}', WriteAttribute::ExternalReference, None),
            "then the externalReference",
        ),
        (
            term("v", "u"),
            bad(GIVEN, '\u{3}', WriteAttribute::NameContent, None),
            "then the name",
        ),
    ] {
        assert_eq!(
            refused(doc, operation),
            expected,
            "{ROW}: CreateTerm: {what}"
        );
    }
}

// ---------------------------------------------------------------------------
// The characters
// ---------------------------------------------------------------------------

/// The characters outside XML 1.0's `Char` production: the 29 C0 controls
/// other than TAB, LF and CR, and U+FFFE and U+FFFF.
fn outside_char() -> Vec<char> {
    (0_u32..0x20)
        .filter(|code| ![0x9, 0xA, 0xD].contains(code))
        .chain([0xFFFE, 0xFFFF])
        .filter_map(char::from_u32)
        .collect()
}

/// The edges of XML 1.0's `Char` production, each a character it admits.
const CHAR_EDGES: [char; 15] = [
    '\t',
    '\n',
    '\r',
    '\u{20}',
    '\u{7F}',
    '\u{85}',
    '\u{9F}',
    '\u{D7FF}',
    '\u{E000}',
    '\u{FDD0}',
    '\u{FFFD}',
    '\u{10000}',
    '\u{1F600}',
    '\u{1FFFE}',
    '\u{10FFFF}',
];

#[test]
fn lib_edit_025_every_character_outside_xml_char_is_refused() {
    let characters = outside_char();
    assert_eq!(characters.len(), 31, "{ROW}: setup: not 29 controls and 2");
    let mut doc = start();
    for character in characters {
        let code = u32::from(character);
        assert_eq!(
            refused(
                &mut doc,
                SetName::new("c1", LangString::new(format!("a{character}b")))
            ),
            bad("c1", character, WriteAttribute::NameContent, None),
            "{ROW}: U+{code:04X} in a name"
        );
        assert_eq!(
            refused(&mut doc, SetGid::new("c1", character.to_string())),
            bad("c1", character, WriteAttribute::Gid, None),
            "{ROW}: U+{code:04X} in a gid"
        );
    }
}

#[test]
fn lib_edit_025_every_xml_char_edge_is_accepted_and_reads_back() {
    for character in CHAR_EDGES {
        let code = u32::from(character);
        let mut doc = start();
        let name = LangString::new(format!("a{character}b")).with_lang(format!("e{character}"));
        let gid = format!("g{character}");
        let value = format!("v{character}");
        let content = MultiLangString::from(LangString::new(format!("x{character}")));
        accepted(&mut doc, SetName::new("c1", name.clone()));
        accepted(&mut doc, SetGid::new("c1", gid.clone()));
        accepted(&mut doc, SetValue::new("t1", value.clone()));
        accepted(&mut doc, SetContent::new("n1", content.clone()));
        let held = |doc: &Document| {
            let element = |id: &str| {
                doc.get(id)
                    .unwrap_or_else(|| panic!("{ROW}: the document has no {id}"))
            };
            (
                element("c1").name().cloned(),
                element("c1").gid().map(str::to_owned),
                element("t1").value().map(str::to_owned),
                element("n1").content().cloned(),
            )
        };
        assert_eq!(
            held(&doc),
            (Some(name), Some(gid), Some(value), Some(content)),
            "{ROW}: U+{code:04X} is not held exactly"
        );
        for mode in MODES {
            let what = format!("U+{code:04X} ({mode:?})");
            let back = read_back(&what, &written(&what, &doc, mode));
            assert!(back == doc, "{ROW}: {what} does not read back equal");
        }
    }
}

// ---------------------------------------------------------------------------
// The place of the check
// ---------------------------------------------------------------------------

/// A name `ExpressionLangString` with no expression, read from a file, whose
/// content holds U+0001.
fn expression_name_without_expression() -> LangString {
    let input = format!(
        "<sacm:ArgumentPackage{NS} xmi:id=\"p\">\
         <name xmi:type=\"sacm:ExpressionLangString\" lang=\"en\" content=\"x\"/>\
         </sacm:ArgumentPackage>"
    );
    let doc = read_back("an ExpressionLangString with no expression", &input);
    let mut name = doc
        .get("p")
        .and_then(|package| package.name().cloned())
        .unwrap_or_else(|| panic!("{ROW}: setup: the name with no expression is not read"));
    assert!(
        name.is_expression_lang_string() && name.expression().is_none(),
        "{ROW}: setup: the name is not an ExpressionLangString with no expression: {name:?}"
    );
    name.set_content(Some("x\u{1}".to_owned()));
    name
}

#[test]
fn lib_edit_025_the_refusal_is_the_last_check() {
    let mut doc = start();
    let bad_name = || LangString::new("bad\u{1}");
    // (what, the operation, the variant that wins)
    let cases: Vec<(&str, Operation, &str)> = vec![
        (
            "ParentNotFound",
            CreateClaim::new(NOWHERE, bad_name()).into(),
            "ParentNotFound",
        ),
        (
            "InvalidParent",
            CreateClaim::new("n1", bad_name()).into(),
            "InvalidParent",
        ),
        (
            "InvalidId",
            CreateClaim::new("p", bad_name()).with_id("1c").into(),
            "InvalidId",
        ),
        (
            "DuplicateId",
            CreateClaim::new("p", bad_name()).with_id("c1").into(),
            "DuplicateId",
        ),
        (
            "ElementNotFound",
            SetName::new(NOWHERE, bad_name()).into(),
            "ElementNotFound",
        ),
        (
            "FeatureNotApplicable",
            SetVersion::new("c1", "v\u{1}").into(),
            "FeatureNotApplicable",
        ),
        (
            "FeatureNotApplicable (a name on a Note)",
            SetName::new("n1", bad_name()).into(),
            "FeatureNotApplicable",
        ),
        (
            "TooFewReferences (a relationship with no source)",
            CreateAssertedInference::new("p", bad_name(), Vec::<ElementId>::new(), "c2").into(),
            "TooFewReferences",
        ),
        (
            "TooFewReferences (an expression name without its expression)",
            CreateClaim::new("p", expression_name_without_expression()).into(),
            "TooFewReferences",
        ),
        (
            "TooFewReferences (SetName)",
            SetName::new("c1", expression_name_without_expression()).into(),
            "TooFewReferences",
        ),
        (
            "ReferenceNotFound (a source)",
            CreateAssertedInference::new("p", bad_name(), [NOWHERE], "c2").into(),
            "ReferenceNotFound",
        ),
        (
            "ReferenceNotFound (an expression)",
            CreateClaim::new("p", bad_name().with_expression(NOWHERE)).into(),
            "ReferenceNotFound",
        ),
        (
            "WrongReferenceType (an expression)",
            CreateClaim::new("p", bad_name().with_expression("c1")).into(),
            "WrongReferenceType",
        ),
    ];
    for (what, operation, variant) in cases {
        let error = refusal(&mut doc, &operation);
        let shown = format!("{error:?}");
        assert!(
            shown.starts_with(&format!("{variant} ")),
            "{ROW}: {what}: {variant} does not win over a bad character: {error:?}"
        );
    }
}

/// One operation per kind of text, each refused for a bad character.
fn sample_refusals() -> Vec<Operation> {
    let bad = || LangString::new("bad\u{1}");
    vec![
        CreateClaim::new("p", bad()).into(),
        CreateClaim::new("p", bad()).with_id(GIVEN).into(),
        CreateArtifact::new("a", named("A"))
            .with_version("v\u{1}")
            .into(),
        CreateEvent::new("a", named("E")).with_date("d\u{1}").into(),
        CreateActivity::new("a", named("A"))
            .with_end_time("e\u{1}")
            .into(),
        CreateResource::new("a", named("R"))
            .with_location(multi(&[named("l"), bad()]))
            .into(),
        CreateTerm::new("terms", named("T"), "v")
            .with_external_reference("u\u{1}")
            .into(),
        CreateExpression::new("terms", named("E"), "v\u{1}").into(),
        CreateDescription::new("c1", bad()).into(),
        CreateImplementationConstraint::new("c1", bad()).into(),
        CreateNote::new("c1", multi(&[named("ok"), bad()])).into(),
        CreateTaggedValue::new("c1", bad(), named("v")).into(),
        SetName::new("c1", LangString::new("x").with_lang("\u{1}")).into(),
        SetGid::new("p", "g\u{1}").into(),
        SetContent::new("n1", multi(&[named("ok"), bad()])).into(),
        SetKey::new("tv1", bad()).into(),
        SetTaggedValue::new("c2", "k\u{1}", "v").into(),
        SetTaggedValue::new("c1", "k", multi(&[named("ok"), bad()])).into(),
        SetVersion::new("report", "v\u{1}").into(),
        SetDate::new("ev1", "d\u{1}").into(),
        SetStartTime::new("act1", "s\u{1}").into(),
        SetEndTime::new("act1", "e\u{1}").into(),
        SetLocation::new("r1", multi(&[bad()])).into(),
        SetValue::new("e1", "v\u{1}").into(),
        SetExternalReference::new("t1", "u\u{1}").into(),
    ]
}

#[test]
fn lib_edit_025_check_preview_and_apply_agree_and_a_refusal_changes_nothing() {
    let good = || Operation::from(CreateClaim::new("p", named("Fine")).with_id("fine"));
    for operation in sample_refusals() {
        let mut doc = start();
        let (before, revision, next) = (doc.clone(), doc.revision(), next_ids(&doc));
        let unchanged = |doc: &Document, how: &str| {
            assert!(
                *doc == before && doc.revision() == revision && next_ids(doc) == next,
                "{ROW}: {how} of {operation:?} changed the document, its revision or its next \
                 generated ids"
            );
        };
        // check, preview and apply (`refused` compares the three answers).
        let error = refusal(&mut doc, &operation);
        assert!(
            unrepresentable(&error).is_some(),
            "{ROW}: {operation:?} is not refused as UnrepresentableCharacter: {error:?}"
        );
        unchanged(&doc, "apply");
        // Batch::apply, after an accepted operation.
        let answer = edit::batch(&mut doc, |tx| -> Result<(), BatchError> {
            tx.apply(good())?;
            tx.apply(operation.clone())?;
            Ok(())
        });
        let assert_batch = |answer: Result<_, BatchError>, how: &str| match answer {
            Ok(batch) => panic!("{ROW}: {how} accepts {operation:?}: {batch:?}"),
            Err(failure) => assert_eq!(
                (failure.index(), failure.error()),
                (1, &error),
                "{ROW}: {how} does not refuse {operation:?} at index 1 with apply's refusal"
            ),
        };
        assert_batch(answer, "Batch::apply");
        unchanged(&doc, "Batch::apply");
        assert_batch(
            edit::apply_all(&mut doc, [good(), operation.clone()]),
            "apply_all",
        );
        unchanged(&doc, "apply_all");
        assert_batch(
            edit::preview_batch(&doc, |tx| -> Result<(), BatchError> {
                tx.apply(good())?;
                tx.apply(operation.clone())?;
                Ok(())
            }),
            "preview_batch",
        );
        assert_batch(
            edit::preview_all(&doc, [good(), operation.clone()]),
            "preview_all",
        );
        unchanged(&doc, "preview_batch and preview_all");
    }
}

#[test]
fn lib_edit_025_an_operation_that_stores_nothing_is_not_refused() {
    let mut doc = start();
    // The seam gives each text of the start document a bad character.
    let bad = || LangString::new("bad\u{1}");
    seam(&mut doc, CreateClaim::new("p", bad()).with_id("cb"));
    let held: Vec<Operation> = vec![
        SetName::new("c1", bad()).into(),
        SetGid::new("c1", "g\u{1}").into(),
        SetVersion::new("report", "v\u{1}").into(),
        SetDate::new("report", "d\u{1}").into(),
        SetDate::new("ev1", "d\u{1}").into(),
        SetStartTime::new("act1", "s\u{1}").into(),
        SetEndTime::new("act1", "e\u{1}").into(),
        SetValue::new("t1", "v\u{1}").into(),
        SetExternalReference::new("t1", "u\u{1}").into(),
        SetContent::new("n1", multi(&[named("ok"), bad()])).into(),
        SetContent::new("tv1", multi(&[bad()])).into(),
        SetLocation::new("r1", multi(&[bad()])).into(),
    ];
    for operation in &held {
        seam(&mut doc, operation.clone());
    }
    // A set of the value held stores nothing: an empty mutation.
    let mut same = held;
    same.push(SetTaggedValue::new("c1", "k", multi(&[bad()])).into());
    for operation in same {
        let mutation = accepted(&mut doc, operation.clone());
        assert!(
            mutation.is_empty(),
            "{ROW}: a set of the value held, {operation:?}, is not an empty mutation: \
             {mutation:?}"
        );
    }
    // The removes, flags, declarations, references, reorders, moves, copies
    // and deletes store no new text.
    let copy = CopyElements::new(&doc, ["cb"], "q")
        .unwrap_or_else(|error| panic!("{ROW}: setup: the copy is not built: {error}"));
    let rest: Vec<Operation> = vec![
        SetGid::remove("c1").into(),
        SetVersion::remove("report").into(),
        SetDate::remove("report").into(),
        SetDate::remove("ev1").into(),
        SetStartTime::remove("act1").into(),
        SetEndTime::remove("act1").into(),
        SetExternalReference::remove("t1").into(),
        SetContent::remove("n1").into(),
        SetLocation::remove("r1").into(),
        SetFlag::new("cb", Flag::IsAbstract, true).into(),
        SetAssertionDeclaration::new("cb", AssertionDeclaration::Assumed).into(),
        AddReference::new("c2", ReferenceEnd::MetaClaim, "cb").into(),
        ReorderElements::new("p", ["cb", "c2", "c1"]).into(),
        MoveElement::new("cb", "q").into(),
        copy.into(),
        DeleteElement::new("cb")
            .with_reference_policy(ReferenceDeletePolicy::ScrubReferences)
            .with_cross_package_policy(CrossPackageReferencePolicy::ApplyReferencePolicy)
            .into(),
        DeleteElement::new("c1")
            .with_package_policy(PackageDeletePolicy::DeleteRecursively)
            .into(),
    ];
    for operation in rest {
        let name = operation.operation_name();
        let mutation = accepted(&mut doc, operation);
        assert!(
            !mutation.is_empty(),
            "{ROW}: {name} on an element holding a bad character changed nothing"
        );
    }
}

#[test]
fn lib_edit_025_an_edit_names_what_the_write_would_refuse() {
    let start = start();
    let mut operations = sample_refusals();
    // Several bad texts in one operation: the first is named by both.
    operations.extend([
        Operation::from(
            CreateArtifact::new("a", LangString::new("n\u{3}"))
                .with_version("v\u{1}")
                .with_date("d\u{2}"),
        ),
        CreateTerm::new("terms", LangString::new("n\u{3}").with_lang("\u{4}"), "v")
            .with_external_reference("u\u{2}")
            .into(),
        CreateResource::new("a", LangString::new("n").with_lang("\u{4}"))
            .with_location(multi(&[LangString::new("x\u{2}").with_lang("\u{1}")]))
            .into(),
        CreateTaggedValue::new("c1", LangString::new("k\u{1}"), LangString::new("c\u{2}")).into(),
        SetTaggedValue::new("c2", "k\u{1}", LangString::new("c").with_lang("\u{2}")).into(),
    ]);
    for operation in operations {
        let edit_refusal = refused(&mut start.clone(), operation.clone());
        let mut held = start.clone();
        seam(&mut held, operation.clone());
        assert_eq!(
            edit_refusal,
            write_refusal(&held),
            "{ROW}: the refusal of {operation:?} names otherwise than the write"
        );
    }
}

// ---------------------------------------------------------------------------
// The code, the message and the JSON
// ---------------------------------------------------------------------------

/// The cells of the catalog row of `code`, which must be the only one.
fn catalog_row(code: &str) -> Vec<String> {
    let path = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../..")
        .join(CATALOG);
    let catalog = std::fs::read_to_string(&path)
        .unwrap_or_else(|error| panic!("{ROW}: setup: read {}: {error}", path.display()));
    let rows: Vec<Vec<String>> = catalog
        .lines()
        .filter_map(|line| line.trim().strip_prefix('|'))
        .map(|line| line.split('|').map(|cell| cell.trim().to_owned()).collect())
        .filter(|cells: &Vec<String>| cells.first().is_some_and(|first| first == code))
        .collect();
    match <[Vec<String>; 1]>::try_from(rows) {
        Ok([row]) => row,
        Err(rows) => panic!(
            "{ROW}: {CATALOG} has {} rows for {code}, not one",
            rows.len()
        ),
    }
}

#[test]
fn lib_edit_025_the_refusal_has_code_sacm_xml_003_and_the_catalog_names_it() {
    let error = refusal(
        &mut start(),
        &SetName::new("c1", LangString::new("bad\u{1}")).into(),
    );
    assert_eq!(
        (error.code(), error.code().as_str()),
        (Code::XML_UNREPRESENTABLE_CHARACTER, "SACM-XML-003"),
        "{ROW}: the code of the refusal"
    );
    let as_error: &dyn std::error::Error = &error;
    assert!(
        as_error.source().is_none(),
        "{ROW}: the refusal has a source"
    );
    let row = catalog_row("SACM-XML-003");
    let (severity, meaning) = (
        row.get(1).map_or("", String::as_str),
        row.get(2).map_or("", String::as_str),
    );
    assert_eq!(severity, "Error", "{ROW}: the severity of SACM-XML-003");
    for needle in [
        "`EditError::UnrepresentableCharacter`",
        "`WriteError::UnrepresentableCharacter`",
        "`Code::XML_UNREPRESENTABLE_CHARACTER`",
        "LIB-EDIT-025",
    ] {
        assert!(
            meaning.contains(needle),
            "{ROW}: the {CATALOG} row of SACM-XML-003 does not name {needle}: {meaning:?}"
        );
    }
}

/// The two refusals of the golden samples: U+0001 in a new package's name,
/// and U+0004 in the second value of the Note `n1`'s content.
fn golden_refusals() -> [EditError; 2] {
    let in_name = refusal(
        &mut Document::new(),
        &CreateArgumentPackage::root(LangString::new("bad\u{1}"))
            .with_id("arguments")
            .into(),
    );
    let mut doc = Document::new();
    accepted(
        &mut doc,
        CreateArgumentPackage::root(LangString::new("P")).with_id("p"),
    );
    accepted(
        &mut doc,
        CreateNote::new("p", LangString::new("ok")).with_id("n1"),
    );
    let in_value = refusal(
        &mut doc,
        &SetContent::new(
            "n1",
            multi(&[LangString::new("ok"), LangString::new("bad\u{4}")]),
        )
        .into(),
    );
    [in_name, in_value]
}

#[test]
fn lib_edit_025_display_and_json_match_the_golden_samples() {
    let [in_name, in_value] = golden_refusals();
    assert_eq!(
        (unrepresentable(&in_name), unrepresentable(&in_value)),
        (
            Some(bad("arguments", '\u{1}', WriteAttribute::NameContent, None)),
            Some(bad(
                "n1",
                '\u{4}',
                WriteAttribute::ContentValueContent,
                Some(1)
            )),
        ),
        "{ROW}: the golden refusals"
    );
    assert_eq!(
        in_name.to_string(),
        "the name's content given to the element \"arguments\" holds U+0001, which XML 1.0 \
         cannot represent",
        "{ROW}: the Display without a position"
    );
    assert_eq!(
        in_value.to_string(),
        "the content value's content at index 1 given to the element \"n1\" holds U+0004, \
         which XML 1.0 cannot represent",
        "{ROW}: the Display with a position"
    );
    assert_json(&[in_name, in_value]);
}

/// The compact JSON of the two golden refusals.
#[cfg(feature = "serde")]
const GOLDEN_JSON: [&str; 2] = [
    r#"{"type":"UnrepresentableCharacter","code":"SACM-XML-003","element":"arguments","character":"\u0001","attribute":"NameContent","position":null}"#,
    r#"{"type":"UnrepresentableCharacter","code":"SACM-XML-003","element":"n1","character":"\u0004","attribute":"ContentValueContent","position":1}"#,
];

/// With `serde`, each refusal serializes as its sample, which reads back
/// equal, and `edit-errors.json` holds the two, in order, from its first
/// `UnrepresentableCharacter` sample; a wrong code and a missing `position`
/// are refused.
#[cfg(feature = "serde")]
fn assert_json(refusals: &[EditError; 2]) {
    for (error, sample) in refusals.iter().zip(GOLDEN_JSON) {
        let written = serde_json::to_string(error)
            .unwrap_or_else(|failure| panic!("{ROW}: {error:?} does not serialize: {failure}"));
        assert_eq!(written, sample, "{ROW}: the JSON of {error:?}");
        let read: EditError = serde_json::from_str(sample)
            .unwrap_or_else(|failure| panic!("{ROW}: {sample} does not read: {failure}"));
        assert_eq!(&read, error, "{ROW}: {sample} does not read back equal");
        // `position` is the last key: without it, the record ends after
        // `attribute`.
        let Some((head, _)) = sample.split_once(",\"position\":") else {
            panic!("{ROW}: setup: {sample} has no position");
        };
        let changes = [
            (
                "a wrong code",
                sample.replacen("\"SACM-XML-003\"", "\"SACM-CMD-001\"", 1),
            ),
            (
                "another SACM-XML code",
                sample.replacen("\"SACM-XML-003\"", "\"SACM-XML-001\"", 1),
            ),
            ("a missing position", format!("{head}}}")),
        ];
        for (what, changed) in changes {
            assert_ne!(changed, sample, "{ROW}: setup: {what} changes nothing");
            match serde_json::from_str::<EditError>(&changed) {
                Ok(value) => panic!("{ROW}: {what}: {changed} was read as {value:?}"),
                Err(failure) => assert!(
                    failure.is_data(),
                    "{ROW}: {what}: {changed} is refused as malformed JSON: {failure}"
                ),
            }
        }
    }
    let path =
        Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/data/golden/serde/edit-errors.json");
    let golden = std::fs::read_to_string(&path)
        .unwrap_or_else(|failure| panic!("{ROW}: setup: read {}: {failure}", path.display()));
    let samples: Vec<EditError> = serde_json::from_str(&golden)
        .unwrap_or_else(|failure| panic!("{ROW}: edit-errors.json does not read: {failure}"));
    // The two are no longer the last (slice 0090 appends three).
    let Some(first) = samples
        .iter()
        .position(|sample| unrepresentable(sample).is_some())
    else {
        panic!("{ROW}: edit-errors.json holds no UnrepresentableCharacter sample");
    };
    assert_eq!(
        samples.get(first..first + 2),
        Some(refusals.as_slice()),
        "{ROW}: edit-errors.json does not hold the two refusals, in order"
    );
}

/// Without `serde`, there is no JSON to check.
#[cfg(not(feature = "serde"))]
fn assert_json(_refusals: &[EditError; 2]) {}

#[cfg(feature = "serde")]
#[test]
fn lib_edit_025_an_operation_read_from_json_is_refused_alike() {
    let json = r#"{"type":"SetName","element":"c1","name":{"lang":"en","content":"bad\u0001"}}"#;
    let operation: Operation = serde_json::from_str(json)
        .unwrap_or_else(|failure| panic!("{ROW}: {json} does not read: {failure}"));
    let twin = Operation::from(SetName::new(
        "c1",
        LangString::new("bad\u{1}").with_lang("en"),
    ));
    assert_eq!(operation, twin, "{ROW}: the JSON does not read as its twin");
    let mut doc = start();
    assert_eq!(
        refused(&mut doc, operation),
        bad("c1", '\u{1}', WriteAttribute::NameContent, None),
        "{ROW}: the operation read from JSON"
    );
    assert_eq!(
        refused(&mut doc, twin),
        bad("c1", '\u{1}', WriteAttribute::NameContent, None),
        "{ROW}: its constructed twin"
    );
    // A list read from JSON, applied with apply_all.
    let list = r#"[{"type":"SetGid","element":"c1","gid":"g"},{"type":"SetGid","element":"c1","gid":"g\u0002"}]"#;
    let operations: Vec<Operation> = serde_json::from_str(list)
        .unwrap_or_else(|failure| panic!("{ROW}: {list} does not read: {failure}"));
    let before = doc.clone();
    match edit::apply_all(&mut doc, operations) {
        Ok(batch) => panic!("{ROW}: apply_all accepts a gid holding U+0002: {batch:?}"),
        Err(failure) => assert_eq!(
            (failure.index(), unrepresentable(failure.error())),
            (1, Some(bad("c1", '\u{2}', WriteAttribute::Gid, None))),
            "{ROW}: apply_all of the list read from JSON"
        ),
    }
    assert!(
        doc == before,
        "{ROW}: the refused list changed the document"
    );
}

// ---------------------------------------------------------------------------
// A read never holds such text
// ---------------------------------------------------------------------------

#[test]
fn lib_edit_025_a_read_never_holds_a_character_xml_cannot_represent() {
    // A literal character, and character references, in a name's content, a
    // gid and a Term's value.
    let texts = [
        "\u{1}", "\u{0}", "\u{FFFE}", "\u{FFFF}", "&#1;", "&#0;", "&#x1F;", "&#xFFFE;", "&#xFFFF;",
    ];
    for text in texts {
        let inputs = [
            (
                "a name's content",
                format!(
                    "<sacm:ArgumentPackage{NS} xmi:id=\"p\"><name content=\"a{text}b\"/>\
                     </sacm:ArgumentPackage>"
                ),
            ),
            (
                "a gid",
                format!(
                    "<sacm:ArgumentPackage{NS} xmi:id=\"p\" gid=\"a{text}b\"><name content=\"P\"/>\
                     </sacm:ArgumentPackage>"
                ),
            ),
            (
                "a Term's value",
                format!(
                    "<sacm:TerminologyPackage{NS} xmi:id=\"terms\"><name content=\"T\"/>\
                     <terminologyElement xmi:type=\"sacm:Term\" xmi:id=\"t1\" value=\"a{text}b\">\
                     <name content=\"t\"/></terminologyElement></sacm:TerminologyPackage>"
                ),
            ),
        ];
        for (what, input) in inputs {
            for mode in MODES {
                match xmi::read(input.as_bytes(), &ReadOptions::default().with_mode(mode)) {
                    Err(error @ ReadError::Malformed { .. }) => assert_eq!(
                        error.code(),
                        Code::XML_MALFORMED,
                        "{ROW}: {text:?} in {what} ({mode:?}): the code"
                    ),
                    Err(other) => {
                        panic!("{ROW}: {text:?} in {what} ({mode:?}) is not Malformed: {other:?}")
                    }
                    Ok(output) => panic!(
                        "{ROW}: {text:?} in {what} ({mode:?}) is read: {:?}",
                        output.document()
                    ),
                }
            }
        }
    }
}

// ---------------------------------------------------------------------------
// A document built by accepted edits always writes
// ---------------------------------------------------------------------------

/// The number of random sequences from an empty document.
const SEEDS: u64 = 64;

/// The operations of each random sequence.
const STEPS: usize = 40;

/// The random sequences from each golden fixture.
const GOLDEN_SEEDS: u64 = 4;

/// Texts XML 1.0 can represent, markup, line ends and edges included.
const GOOD_TEXTS: [&str; 6] = [
    "",
    "ok",
    "S\u{e4}kerhet \u{2713} <&>\"'",
    "tab\there\r\nline\rcr\n",
    "\u{85}\u{2028}\u{FFFD}\u{1F600}\u{10FFFF}",
    "2024-03-01",
];

/// Texts holding a character XML 1.0 cannot represent.
const BAD_TEXTS: [&str; 6] = [
    "a\u{1}",
    "\u{0}",
    "x\u{FFFE}y",
    "\u{1F}",
    "ok\u{FFFF}",
    "\u{B}\u{C}",
];

/// The text operations the random sequences must have both accepted and
/// refused for a bad character.
const TEXT_OPERATIONS: [&str; 26] = [
    "CreateAssuranceCasePackage",
    "CreateArgumentPackage",
    "CreateArtifactPackage",
    "CreateTerminologyPackage",
    "CreateClaim",
    "CreateDescription",
    "CreateNote",
    "CreateTaggedValue",
    "CreateArtifact",
    "CreateEvent",
    "CreateResource",
    "CreateActivity",
    "CreateTerm",
    "CreateExpression",
    "SetName",
    "SetGid",
    "SetContent",
    "SetKey",
    "SetTaggedValue",
    "SetVersion",
    "SetDate",
    "SetStartTime",
    "SetEndTime",
    "SetLocation",
    "SetValue",
    "SetExternalReference",
];

/// `SplitMix64`, as in `batch.rs`: the same on every platform and run.
struct Rng(u64);

impl Rng {
    fn next_u64(&mut self) -> u64 {
        self.0 = self.0.wrapping_add(0x9E37_79B9_7F4A_7C15);
        let mut z = self.0;
        z = (z ^ (z >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
        z = (z ^ (z >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
        z ^ (z >> 31)
    }

    /// A number below `bound`; 0 when `bound` is 0.
    fn below(&mut self, bound: usize) -> usize {
        let Ok(wide) = u64::try_from(bound) else {
            return 0;
        };
        if wide == 0 {
            return 0;
        }
        usize::try_from(self.next_u64() % wide).unwrap_or(0)
    }

    fn flip(&mut self) -> bool {
        self.below(2) == 0
    }

    /// One of `items`, or `None` when there are none.
    fn pick<'a, T>(&mut self, items: &'a [T]) -> Option<&'a T> {
        items.get(self.below(items.len()))
    }

    /// A text: one in three bad.
    fn text(&mut self) -> String {
        let pool: &[&str] = if self.below(3) == 0 {
            &BAD_TEXTS
        } else {
            &GOOD_TEXTS
        };
        (*self.pick(pool).unwrap_or(&"")).to_owned()
    }

    /// A `LangString`: a content, and half the time a lang.
    fn lang_string(&mut self) -> LangString {
        let value = LangString::new(self.text());
        if self.flip() {
            value.with_lang(self.text())
        } else {
            value
        }
    }

    /// One or two values.
    fn multi(&mut self) -> MultiLangString {
        let count = 1 + self.below(2);
        (0..count).map(|_| self.lang_string()).collect()
    }

    /// An id of an element of `doc` of a kind in `kinds` three times in
    /// four, otherwise of any element; [`NOWHERE`] when there is none.
    fn id_of(&mut self, doc: &Document, kinds: &[ElementKind]) -> ElementId {
        let of_kind: Vec<ElementId> = doc
            .elements()
            .filter(|element| kinds.contains(&element.kind()))
            .map(|element| element.id().clone())
            .collect();
        if !of_kind.is_empty() && self.below(4) != 0 {
            return self
                .pick(&of_kind)
                .cloned()
                .unwrap_or_else(|| ElementId::from(NOWHERE));
        }
        let all: Vec<ElementId> = doc.elements().map(|element| element.id().clone()).collect();
        self.pick(&all)
            .cloned()
            .unwrap_or_else(|| ElementId::from(NOWHERE))
    }
}

const ARGUMENT_PACKAGES: [ElementKind; 1] = [ElementKind::ArgumentPackage];
const ARTIFACT_PACKAGES: [ElementKind; 1] = [ElementKind::ArtifactPackage];
const TERMINOLOGY_PACKAGES: [ElementKind; 1] = [ElementKind::TerminologyPackage];
const CASE_PACKAGES: [ElementKind; 1] = [ElementKind::AssuranceCasePackage];

/// One random operation for the state `doc`; `None` when a copy cannot be
/// built.
fn random_operation(rng: &mut Rng, doc: &Document, fresh: &mut usize) -> Option<Operation> {
    let caller = if rng.below(5) == 0 {
        *fresh += 1;
        Some(format!("x{fresh}"))
    } else {
        None
    };
    let id = caller.as_deref();
    let name = rng.lang_string();
    // Without a root, most operations have nowhere to go.
    let pick = if doc.roots().next().is_none() {
        0
    } else {
        rng.below(20)
    };
    let operation = match pick {
        0 => match rng.below(4) {
            0 => with_caller!(CreateAssuranceCasePackage::root(name), id),
            1 => with_caller!(CreateArgumentPackage::root(name), id),
            2 => with_caller!(CreateArtifactPackage::root(name), id),
            _ => with_caller!(CreateTerminologyPackage::root(name), id),
        },
        1 => {
            let case = rng.id_of(doc, &CASE_PACKAGES);
            match rng.below(4) {
                0 => with_caller!(CreateAssuranceCasePackage::new(case, name), id),
                1 => with_caller!(CreateArgumentPackage::new(case, name), id),
                2 => with_caller!(CreateArtifactPackage::new(case, name), id),
                _ => with_caller!(CreateTerminologyPackage::new(case, name), id),
            }
        }
        2 => random_argumentation_create(rng, doc, name, id),
        3 => random_utility_create(rng, doc, id),
        4..=7 => random_artifact_create(rng, doc, name, id),
        8 | 9 => random_terminology_create(rng, doc, name, id),
        10..=17 => random_set(rng, doc),
        18 => DeleteElement::new(rng.id_of(doc, &[]))
            .with_reference_policy(ReferenceDeletePolicy::DeleteReferencingRelationships)
            .with_package_policy(PackageDeletePolicy::DeleteRecursively)
            .with_cross_package_policy(CrossPackageReferencePolicy::ApplyReferencePolicy)
            .into(),
        _ if rng.flip() => {
            let element = rng.id_of(doc, &[]);
            if rng.below(4) == 0 {
                MoveElement::root(element).into()
            } else {
                MoveElement::new(element, rng.id_of(doc, &[])).into()
            }
        }
        _ => {
            let element = rng.id_of(doc, &[]);
            let parent = rng.id_of(doc, &[]);
            CopyElements::new(doc, [element], parent).ok()?.into()
        }
    };
    Some(operation)
}

fn random_argumentation_create(
    rng: &mut Rng,
    doc: &Document,
    name: LangString,
    id: Option<&str>,
) -> Operation {
    let parent = rng.id_of(doc, &ARGUMENT_PACKAGES);
    match rng.below(5) {
        0 | 1 => with_caller!(CreateClaim::new(parent, name), id),
        2 => with_caller!(CreateArgumentReasoning::new(parent, name), id),
        3 => with_caller!(CreateArtifactReference::new(parent, name), id),
        _ => {
            let source = rng.id_of(doc, &[ElementKind::Claim]);
            let target = rng.id_of(doc, &[ElementKind::Claim]);
            with_caller!(
                CreateAssertedInference::new(parent, name, [source], target),
                id
            )
        }
    }
}

fn random_utility_create(rng: &mut Rng, doc: &Document, id: Option<&str>) -> Operation {
    let parent = rng.id_of(doc, &[]);
    let content = rng.multi();
    match rng.below(4) {
        0 => with_caller!(CreateDescription::new(parent, content), id),
        1 => with_caller!(CreateImplementationConstraint::new(parent, content), id),
        2 => with_caller!(CreateNote::new(parent, content), id),
        _ => with_caller!(CreateTaggedValue::new(parent, rng.multi(), content), id),
    }
}

fn random_artifact_create(
    rng: &mut Rng,
    doc: &Document,
    name: LangString,
    id: Option<&str>,
) -> Operation {
    let parent = rng.id_of(doc, &ARTIFACT_PACKAGES);
    match rng.below(4) {
        0 => {
            let mut create = CreateArtifact::new(parent, name);
            if rng.flip() {
                create = create.with_version(rng.text());
            }
            if rng.flip() {
                create = create.with_date(rng.text());
            }
            with_caller!(create, id)
        }
        1 => with_caller!(CreateEvent::new(parent, name).with_date(rng.text()), id),
        2 => with_caller!(
            CreateResource::new(parent, name).with_location(rng.multi()),
            id
        ),
        _ => {
            let mut create = CreateActivity::new(parent, name);
            if rng.flip() {
                create = create.with_start_time(rng.text());
            }
            if rng.flip() {
                create = create.with_end_time(rng.text());
            }
            with_caller!(create, id)
        }
    }
}

fn random_terminology_create(
    rng: &mut Rng,
    doc: &Document,
    name: LangString,
    id: Option<&str>,
) -> Operation {
    let parent = rng.id_of(doc, &TERMINOLOGY_PACKAGES);
    match rng.below(3) {
        0 => {
            let mut create = CreateTerm::new(parent, name, rng.text());
            if rng.flip() {
                create = create.with_external_reference(rng.text());
            }
            with_caller!(create, id)
        }
        1 => with_caller!(CreateExpression::new(parent, name, rng.text()), id),
        _ => with_caller!(CreateCategory::new(parent, name), id),
    }
}

/// One random set of a text on a random element, two times in three one of
/// a kind with features of its own (each such kind as likely); three times
/// in four of a feature its kind has.
fn random_set(rng: &mut Rng, doc: &Document) -> Operation {
    let all: Vec<(ElementId, ElementKind)> = doc
        .elements()
        .map(|element| (element.id().clone(), element.kind()))
        .collect();
    // A special kind present in `doc`, each as likely, then one of its
    // elements.
    let present: Vec<ElementKind> = SPECIAL_KINDS
        .into_iter()
        .filter(|kind| all.iter().any(|(_, found)| found == kind))
        .collect();
    let special: Vec<(ElementId, ElementKind)> = match rng.pick(&present) {
        Some(chosen) => all
            .iter()
            .filter(|(_, kind)| kind == chosen)
            .cloned()
            .collect(),
        None => Vec::new(),
    };
    let elements = if !special.is_empty() && rng.below(3) != 0 {
        &special
    } else {
        &all
    };
    let (target, kind) = rng
        .pick(elements)
        .cloned()
        .unwrap_or_else(|| (ElementId::from(NOWHERE), ElementKind::Claim));
    let remove = rng.below(4) == 0;
    let feature = if rng.below(4) == 0 {
        rng.below(11)
    } else {
        own_feature(rng, kind)
    };
    match feature {
        0 => SetName::new(target, rng.lang_string()).into(),
        1 if remove => SetGid::remove(target).into(),
        1 => SetGid::new(target, rng.text()).into(),
        2 if remove => SetContent::remove(target).into(),
        2 => SetContent::new(target, rng.multi()).into(),
        3 => SetKey::new(target, rng.multi()).into(),
        4 => SetTaggedValue::new(target, rng.text(), rng.multi()).into(),
        5 if remove => SetVersion::remove(target).into(),
        5 => SetVersion::new(target, rng.text()).into(),
        6 if remove => SetDate::remove(target).into(),
        6 => SetDate::new(target, rng.text()).into(),
        7 => match (remove, rng.flip()) {
            (true, true) => SetStartTime::remove(target).into(),
            (true, false) => SetEndTime::remove(target).into(),
            (false, true) => SetStartTime::new(target, rng.text()).into(),
            (false, false) => SetEndTime::new(target, rng.text()).into(),
        },
        8 if remove => SetLocation::remove(target).into(),
        8 => SetLocation::new(target, rng.multi()).into(),
        9 => SetValue::new(target, rng.text()).into(),
        _ if remove => SetExternalReference::remove(target).into(),
        _ => SetExternalReference::new(target, rng.text()).into(),
    }
}

/// The kinds with text features of their own.
const SPECIAL_KINDS: [ElementKind; 10] = [
    ElementKind::Artifact,
    ElementKind::Event,
    ElementKind::Activity,
    ElementKind::Resource,
    ElementKind::Term,
    ElementKind::Expression,
    ElementKind::TaggedValue,
    ElementKind::Description,
    ElementKind::ImplementationConstraint,
    ElementKind::Note,
];

/// A feature of [`random_set`] that elements of `kind` have.
fn own_feature(rng: &mut Rng, kind: ElementKind) -> usize {
    let own: &[usize] = match kind {
        ElementKind::Artifact => &[5, 5, 6],
        ElementKind::Event => &[6],
        ElementKind::Activity => &[7],
        ElementKind::Resource => &[8],
        ElementKind::Term => &[9, 10],
        ElementKind::Expression => &[9],
        ElementKind::TaggedValue => &[2, 3],
        ElementKind::Description | ElementKind::ImplementationConstraint | ElementKind::Note => {
            &[2]
        }
        _ => &[0, 1, 4],
    };
    rng.pick(own).copied().unwrap_or(0)
}

/// How often each operation was accepted, and refused for a bad character.
type Tally = BTreeMap<&'static str, (usize, usize)>;

/// Runs [`STEPS`] random operations on `doc` with `seed`. After each
/// accepted one, while the document has a root, it writes in tolerant mode
/// (and with `strict`, in strict mode too, and reads back equal); each
/// refusal leaves the document `==` to before.
fn random_run(label: &str, mut doc: Document, seed: u64, strict: bool, tally: &mut Tally) {
    let mut rng = Rng(seed);
    let mut fresh = 0;
    for step in 0..STEPS {
        let Some(operation) = random_operation(&mut rng, &doc, &mut fresh) else {
            continue;
        };
        let what = format!("{label}, seed {seed}, step {step}, {operation:?}");
        let before = doc.clone();
        let counts = tally.entry(operation.operation_name()).or_default();
        match edit::apply(&mut doc, operation) {
            Ok(_) => counts.0 += 1,
            Err(error) => {
                if unrepresentable(&error).is_some() {
                    counts.1 += 1;
                }
                assert!(
                    doc == before,
                    "{ROW}: {what}: the refusal changed the document"
                );
                continue;
            }
        }
        if doc.roots().next().is_none() {
            continue;
        }
        let modes: &[Mode] = if strict { &MODES } else { &[Mode::Tolerant] };
        for &mode in modes {
            let text = written(&what, &doc, mode);
            if strict {
                let back = read_back(&what, &text);
                assert!(
                    back == doc,
                    "{ROW}: {what}: the {mode:?} output does not read back equal"
                );
            }
        }
    }
}

/// The golden fixtures, of SACM 2.3 and of the compatibility tests.
fn golden_fixtures() -> Vec<std::path::PathBuf> {
    let mut paths = Vec::new();
    for dir in ["tests/data/sacm23/xmi/golden", "tests/data/compat/golden"] {
        let dir = Path::new(env!("CARGO_MANIFEST_DIR")).join(dir);
        let entries = std::fs::read_dir(&dir)
            .unwrap_or_else(|error| panic!("{ROW}: setup: list {}: {error}", dir.display()));
        for entry in entries {
            let path = entry
                .unwrap_or_else(|error| panic!("{ROW}: setup: list {}: {error}", dir.display()))
                .path();
            if path.to_string_lossy().ends_with(".golden.sacm.xmi") {
                paths.push(path);
            }
        }
    }
    paths.sort();
    paths
}

#[test]
fn lib_edit_025_a_document_built_by_accepted_edits_always_writes() {
    let mut tally = Tally::new();
    for seed in 0..SEEDS {
        random_run(
            "from an empty document",
            Document::new(),
            seed,
            true,
            &mut tally,
        );
    }
    let fixtures = golden_fixtures();
    assert!(
        fixtures.len() >= 20,
        "{ROW}: setup: only {} golden fixtures",
        fixtures.len()
    );
    for path in fixtures {
        let input = std::fs::read(&path)
            .unwrap_or_else(|error| panic!("{ROW}: setup: read {}: {error}", path.display()));
        let doc = match xmi::read(&input, &ReadOptions::default().with_mode(Mode::Tolerant)) {
            Ok(output) => output.into_document(),
            Err(error) => panic!("{ROW}: setup: {} does not read: {error}", path.display()),
        };
        let label = path
            .file_name()
            .map_or_else(String::new, |name| name.to_string_lossy().into_owned());
        for seed in 0..GOLDEN_SEEDS {
            random_run(&label, doc.clone(), seed, false, &mut tally);
        }
    }
    // The sequences, from an empty document and from the fixtures, cover
    // every text operation, accepted and refused.
    for operation in TEXT_OPERATIONS {
        let (accepted, refused) = tally.get(operation).copied().unwrap_or_default();
        assert!(
            accepted > 0 && refused > 0,
            "{ROW}: setup: the sequences do not cover {operation}: {accepted} accepted, \
             {refused} refused for a bad character"
        );
    }
    for operation in ["DeleteElement", "MoveElement", "CopyElements"] {
        let (accepted, _) = tally.get(operation).copied().unwrap_or_default();
        assert!(
            accepted > 0,
            "{ROW}: setup: the sequences accept no {operation}"
        );
    }
}
