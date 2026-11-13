//! Tests for atomic edits and edit results (LIB-EDIT-002, LIB-EDIT-003;
//! ADR 0019 and its amendment 1; ADR 0020).
//!
//! - LIB-EDIT-002: every `edit::apply` either succeeds and leaves the
//!   document structurally valid, or refuses and leaves it `==` to a clone
//!   taken before, generating the same next ids.
//! - LIB-EDIT-003: a successful `edit::apply` returns a `Mutation` holding
//!   the operation as given and the ids created, changed and deleted, which
//!   equal what `semantic_compare(before, after)` reports right-only,
//!   per-element and left-only.
//!
//! # The set
//!
//! Every operation of [`operations_for`] is applied to every starting
//! document of [`starting_documents`] twice:
//! - alone, to a fresh clone of the starting document;
//! - in sequence, all of them one after the other to one document, so that
//!   later operations meet the elements and counters earlier ones left.
//!
//! The starting documents are: empty; built with `edit::apply` only; read
//! from each readable XMI fixture; and built with the `test-support`
//! `Document::insert`, with ArtifactPackage parents, interfaces and bindings,
//! Notes, missing names, a set declaration and an id that is not an `NCName`.
//!
//! The operations are, for every element of the starting document and for an
//! id it does not hold ("nowhere"), one create of each kind under it with a
//! generated id, a fresh caller id, an id that is not an `NCName` and the
//! parent's own (duplicate) id; and both root creates with each id form. So
//! the set holds legal operations, and illegal ones with an unknown parent,
//! an illegal parent kind (a Claim under an AssuranceCasePackage, say), an
//! invalid id and a duplicate id.
//!
//! # Structural validity
//!
//! [`assert_structurally_valid`] checks the four properties of LIB-EDIT-002
//! through the public views only:
//! - S1: ids are unique, `get` round-trips each, and
//!   `elements().count() == element_count()`;
//! - S2: parent and child links agree both ways, and `roots()` holds exactly
//!   the parentless elements;
//! - S3: every root is a package, and every child's kind is legal under its
//!   parent's kind, by this file's own containment table (§8.6, §9.2, §10.4,
//!   §11.4, §12.2, §12.6; ADR 0014), not the crate's;
//! - S4: every Assertion (a Claim or one of the five asserted relationships)
//!   has a declaration.
//!
//! "The next generated ids" of a document are observed on a clone: create one
//! element of each kind there with a generated id and see which ids come out
//! (the probe of `edit_create.rs`, copied).
//!
//! Not checked here:
//! - That no public `&mut Document` path other than `edit::apply` exists
//!   outside `test-support` (LIB-EDIT-002 item 3): a test cannot list the
//!   API; the verifier reviews it.
//! - The audit-log example in the `Mutation` rustdoc (LIB-EDIT-003 item 5):
//!   a doctest, checked by review.

use std::collections::{BTreeMap, BTreeSet};
use std::fmt::Debug;
use std::path::Path;

use sacm::compare::{Difference, semantic_compare};
use sacm::diagnostic::Code;
use sacm::edit::{
    self, CreateArgumentPackage, CreateAssuranceCasePackage, CreateClaim, EditError, Mutation,
    Operation,
};
use sacm::model::ElementKind::{
    Activity, ArgumentGroup, ArgumentPackage, ArgumentPackageBinding, ArgumentPackageInterface,
    ArgumentReasoning, Artifact, ArtifactAssetRelationship, ArtifactGroup, ArtifactPackage,
    ArtifactPackageBinding, ArtifactPackageInterface, ArtifactReference, AssertedArtifactContext,
    AssertedArtifactSupport, AssertedContext, AssertedEvidence, AssertedInference,
    AssuranceCasePackage, AssuranceCasePackageBinding, AssuranceCasePackageInterface, Category,
    Claim, Description, Event, Expression, ImplementationConstraint, Note, Participant, Property,
    Resource, TaggedValue, Technique, Term, TerminologyGroup, TerminologyPackage,
    TerminologyPackageBinding, TerminologyPackageInterface,
};
use sacm::model::{
    AssertionDeclaration, Document, ElementId, ElementKind, LangString, SacmElement,
};
use sacm::xmi::{self, ReadOptions};

const EDIT_002: &str = "LIB-EDIT-002";
const EDIT_003: &str = "LIB-EDIT-003";

/// The directory of the XMI fixtures.
const FIXTURES: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/tests/data/sacm23/xmi");

/// The fixtures `xmi::read` reads in this release, relative to [`FIXTURES`].
/// `golden/nested.golden.sacm.xmi` holds kinds the reader refuses, and the
/// files under `invalid/` are refused by design.
const READABLE_FIXTURES: [&str; 8] = [
    "argument-package-root-valid.sacm.xmi",
    "assurance-case-package-root-valid.sacm.xmi",
    "name-third-party-form-valid.sacm.xmi",
    "wrapped-roots-valid.sacm.xmi",
    "golden/named.golden.sacm.xmi",
    "golden/needs-support-claim.golden.sacm.xmi",
    "golden/several-roots.golden.sacm.xmi",
    "golden/single-root.golden.sacm.xmi",
];

/// The parent id that no starting document holds.
const NOWHERE: &str = "nowhere";

/// The caller id that is not an `NCName`: it starts with a digit.
const NOT_AN_NCNAME: &str = "1bad";

// ---------------------------------------------------------------------------
// The containment table, written from the standard
// ---------------------------------------------------------------------------

/// The four `UtilityElement` kinds (§8.7). They contain no elements.
const UTILITY_ELEMENTS: [ElementKind; 4] =
    [Description, ImplementationConstraint, Note, TaggedValue];

/// The 34 `ModelElement` kinds: every kind but the four `UtilityElement`s.
const MODEL_ELEMENTS: [ElementKind; 34] = [
    AssuranceCasePackage,
    AssuranceCasePackageInterface,
    AssuranceCasePackageBinding,
    TerminologyPackage,
    TerminologyPackageInterface,
    TerminologyPackageBinding,
    TerminologyGroup,
    Category,
    Expression,
    Term,
    ArgumentPackage,
    ArgumentPackageInterface,
    ArgumentPackageBinding,
    ArgumentGroup,
    Claim,
    ArgumentReasoning,
    ArtifactReference,
    AssertedInference,
    AssertedEvidence,
    AssertedContext,
    AssertedArtifactSupport,
    AssertedArtifactContext,
    ArtifactPackage,
    ArtifactPackageInterface,
    ArtifactPackageBinding,
    ArtifactGroup,
    Artifact,
    ArtifactAssetRelationship,
    Activity,
    Event,
    Participant,
    Technique,
    Resource,
    Property,
];

/// AssuranceCasePackage, its interface and its binding (§9.2-§9.4).
const ASSURANCE_CASE_PACKAGES: [ElementKind; 3] = [
    AssuranceCasePackage,
    AssuranceCasePackageInterface,
    AssuranceCasePackageBinding,
];

/// ArgumentPackage, its interface and its binding (§11.4-§11.6).
const ARGUMENT_PACKAGES: [ElementKind; 3] = [
    ArgumentPackage,
    ArgumentPackageInterface,
    ArgumentPackageBinding,
];

/// ArtifactPackage, its interface and its binding (§12.2, §12.4, §12.5).
const ARTIFACT_PACKAGES: [ElementKind; 3] = [
    ArtifactPackage,
    ArtifactPackageInterface,
    ArtifactPackageBinding,
];

/// TerminologyPackage, its interface and its binding (§10.4-§10.6).
const TERMINOLOGY_PACKAGES: [ElementKind; 3] = [
    TerminologyPackage,
    TerminologyPackageInterface,
    TerminologyPackageBinding,
];

/// The 12 `ArgumentationElement` kinds: what `argumentElement` admits (§11.4).
const ARGUMENTATION_ELEMENTS: [ElementKind; 12] = [
    ArgumentPackage,
    ArgumentPackageInterface,
    ArgumentPackageBinding,
    ArgumentGroup,
    Claim,
    ArgumentReasoning,
    ArtifactReference,
    AssertedInference,
    AssertedEvidence,
    AssertedContext,
    AssertedArtifactSupport,
    AssertedArtifactContext,
];

/// The 7 `TerminologyElement` kinds: what `terminologyElement` admits (§10.4;
/// `TerminologyGroup` by the declared type, spec defect D9).
const TERMINOLOGY_ELEMENTS: [ElementKind; 7] = [
    TerminologyPackage,
    TerminologyPackageInterface,
    TerminologyPackageBinding,
    TerminologyGroup,
    Category,
    Expression,
    Term,
];

/// The kinds that hold the `property` role: the eight `ArtifactAsset` kinds
/// (§12.6; `ArtifactAssetRelationship` by §12.14 "Superclass `ArtifactAsset`").
const PROPERTY_HOLDERS: [ElementKind; 8] = [
    Artifact,
    ArtifactAssetRelationship,
    Activity,
    Event,
    Participant,
    Technique,
    Resource,
    Property,
];

/// The 12 package kinds, the only kinds a root may be (ADR 0010 D1).
const PACKAGE_KINDS: [ElementKind; 12] = [
    AssuranceCasePackage,
    AssuranceCasePackageInterface,
    AssuranceCasePackageBinding,
    TerminologyPackage,
    TerminologyPackageInterface,
    TerminologyPackageBinding,
    ArgumentPackage,
    ArgumentPackageInterface,
    ArgumentPackageBinding,
    ArtifactPackage,
    ArtifactPackageInterface,
    ArtifactPackageBinding,
];

/// The six `Assertion` kinds (§11.10 (Claim §11.11, `AssertedRelationship`
/// §11.13)): a Claim and the five asserted relationships. Each has an
/// `assertionDeclaration` (§11.10, `[1]`).
const ASSERTIONS: [ElementKind; 6] = [
    Claim,
    AssertedInference,
    AssertedEvidence,
    AssertedContext,
    AssertedArtifactSupport,
    AssertedArtifactContext,
];

/// Every legal (parent kind, child kind) pair, from the 12 element-typed
/// composite roles.
fn containment_table() -> BTreeSet<(ElementKind, ElementKind)> {
    let mut table = BTreeSet::new();
    let mut admit = |parents: &[ElementKind], children: &[ElementKind]| {
        for &parent in parents {
            for &child in children {
                table.insert((parent, child));
            }
        }
    };
    // ModelElement `description`, `implementationConstraint`, `note`,
    // `taggedValue` (§8.6).
    admit(&MODEL_ELEMENTS, &UTILITY_ELEMENTS);
    // AssuranceCasePackage `assuranceCasePackage`, `argumentPackage`,
    // `artifactPackage`, `terminologyPackage` (§9.2), inherited by its
    // interface and binding.
    for family in [
        ASSURANCE_CASE_PACKAGES,
        ARGUMENT_PACKAGES,
        ARTIFACT_PACKAGES,
        TERMINOLOGY_PACKAGES,
    ] {
        admit(&ASSURANCE_CASE_PACKAGES, &family);
    }
    // ArgumentPackage `argumentElement` (§11.4).
    admit(&ARGUMENT_PACKAGES, &ARGUMENTATION_ELEMENTS);
    // TerminologyPackage `terminologyElement` (§10.4).
    admit(&TERMINOLOGY_PACKAGES, &TERMINOLOGY_ELEMENTS);
    // ArtifactPackage `artifactElement`: every ArtifactElement, so every
    // ModelElement kind (§12.2; ADR 0014).
    admit(&ARTIFACT_PACKAGES, &MODEL_ELEMENTS);
    // ArtifactAsset `property` (§12.6).
    admit(&PROPERTY_HOLDERS, &[Property]);
    table
}

// ---------------------------------------------------------------------------
// Structural validity (LIB-EDIT-002), through the public views only
// ---------------------------------------------------------------------------

/// Asserts the four structural checks S1-S4 on `doc`. `row` and `context`
/// start every failure message.
fn assert_structurally_valid(row: &str, context: &str, doc: &Document) {
    let elements: Vec<SacmElement<'_>> = doc.elements().collect();
    assert_ids_are_unique_and_found(row, context, doc, &elements);
    assert_links_agree(row, context, doc, &elements);
    assert_kinds_are_legal(row, context, doc, &elements);
    assert_assertions_have_declarations(row, context, &elements);
}

/// S1: ids are unique, `get` round-trips each, and every element is visited.
fn assert_ids_are_unique_and_found(
    row: &str,
    context: &str,
    doc: &Document,
    elements: &[SacmElement<'_>],
) {
    assert_eq!(
        elements.len(),
        doc.element_count(),
        "{row}: {context}: S1: elements() visits {} elements but element_count() is {}",
        elements.len(),
        doc.element_count()
    );
    let mut seen = BTreeSet::new();
    for element in elements {
        let id = element.id();
        assert!(
            seen.insert(id.as_str()),
            "{row}: {context}: S1: the id {id:?} is held by more than one element"
        );
        let Some(found) = doc.get(id) else {
            panic!("{row}: {context}: S1: get({id:?}) finds nothing");
        };
        assert_eq!(
            found.id(),
            id,
            "{row}: {context}: S1: get({id:?}) finds another id"
        );
        assert_eq!(
            found.kind(),
            element.kind(),
            "{row}: {context}: S1: get({id:?}) finds another kind"
        );
        assert_eq!(
            found.parent().map(SacmElement::id),
            element.parent().map(SacmElement::id),
            "{row}: {context}: S1: get({id:?}) finds another parent"
        );
        assert!(
            doc.contains(id),
            "{row}: {context}: S1: contains({id:?}) is false"
        );
    }
}

/// S2: parent and child links agree both ways, and `roots()` holds exactly
/// the parentless elements.
fn assert_links_agree(row: &str, context: &str, doc: &Document, elements: &[SacmElement<'_>]) {
    for element in elements {
        let id = element.id();
        for child in element.children() {
            assert_eq!(
                child.parent().map(SacmElement::id),
                Some(id),
                "{row}: {context}: S2: {:?} is a child of {id:?} but has another parent",
                child.id()
            );
        }
        if let Some(parent) = element.parent() {
            let listed = parent.children().filter(|child| child.id() == id).count();
            assert_eq!(
                listed,
                1,
                "{row}: {context}: S2: {id:?} names {:?} as its parent, which lists it {listed} times",
                parent.id()
            );
        }
    }
    let roots: Vec<&ElementId> = doc.roots().map(SacmElement::id).collect();
    let parentless: Vec<&ElementId> = elements
        .iter()
        .filter(|element| element.parent().is_none())
        .map(|element| element.id())
        .collect();
    assert_eq!(
        roots, parentless,
        "{row}: {context}: S2: roots() is not exactly the parentless elements"
    );
}

/// S3: every root is a package, and every child's kind is legal under its
/// parent's kind by [`containment_table`].
fn assert_kinds_are_legal(row: &str, context: &str, doc: &Document, elements: &[SacmElement<'_>]) {
    assert_eq!(
        ElementKind::ALL.len(),
        38,
        "{row}: the kinds changed; review this file's containment table"
    );
    let table = containment_table();
    for root in doc.roots() {
        assert!(
            PACKAGE_KINDS.contains(&root.kind()),
            "{row}: {context}: S3: the root {:?} is a {}, not a package",
            root.id(),
            root.kind()
        );
    }
    for element in elements {
        if let Some(parent) = element.parent() {
            assert!(
                table.contains(&(parent.kind(), element.kind())),
                "{row}: {context}: S3: the {} {:?} is under the {} {:?}, which no role admits",
                element.kind(),
                element.id(),
                parent.kind(),
                parent.id()
            );
        }
    }
}

/// S4: every Assertion has a declaration.
fn assert_assertions_have_declarations(row: &str, context: &str, elements: &[SacmElement<'_>]) {
    for element in elements {
        if ASSERTIONS.contains(&element.kind()) {
            assert!(
                element.assertion_declaration().is_some(),
                "{row}: {context}: S4: the {} {:?} has no assertionDeclaration",
                element.kind(),
                element.id()
            );
        }
    }
}

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

fn id(value: &str) -> ElementId {
    ElementId::from(value)
}

fn named(content: &str) -> LangString {
    LangString::new(content).with_lang("en")
}

/// The names the operations take: present, `LangString::default()`, empty
/// content, and non-ASCII. [`operations_for`] rotates them per (parent, id
/// form, kind), so each name form reaches a successful create of each kind.
fn names() -> [LangString; 4] {
    [
        named("New"),
        LangString::default(),
        LangString::new(""),
        LangString::new("Säkerhet \u{2713} <&>").with_lang("sv-SE"),
    ]
}

/// Applies an operation the test expects to succeed during setup, and
/// returns the one id it created.
fn create(row: &str, doc: &mut Document, operation: impl Into<Operation>) -> ElementId {
    match edit::apply(doc, operation) {
        Ok(mutation) => {
            let created = mutation.created_ids();
            let [only] = created else {
                panic!("{row}: setup: created_ids() is {created:?}, not one new element");
            };
            only.clone()
        }
        Err(error) => panic!("{row}: setup: the operation was refused: {error} ({error:?})"),
    }
}

/// Inserts one element through the `test-support` seam, or fails the test as
/// a setup error.
fn insert(
    doc: &mut Document,
    parent: Option<&str>,
    kind: ElementKind,
    value: &str,
    name: Option<LangString>,
) {
    let parent = parent.map(ElementId::from);
    if let Err(error) = doc.insert(parent.as_ref(), kind, id(value), name) {
        panic!("{EDIT_002}: setup could not insert the {kind} {value:?}: {error}");
    }
}

/// The ids the next generated AssuranceCasePackage, ArgumentPackage and Claim
/// would get, observed on a clone so that `doc` is untouched.
fn next_generated_ids(row: &str, doc: &Document) -> [ElementId; 3] {
    let mut probe = doc.clone();
    let case = create(
        row,
        &mut probe,
        CreateAssuranceCasePackage::root(named("probe")),
    );
    let arguments = create(row, &mut probe, CreateArgumentPackage::root(named("probe")));
    let claim = create(
        row,
        &mut probe,
        CreateClaim::new(&arguments, named("probe")),
    );
    [case, arguments, claim]
}

/// The position of `kind` in [`next_generated_ids`].
fn probe_index(kind: ElementKind) -> usize {
    match kind {
        AssuranceCasePackage => 0,
        ArgumentPackage => 1,
        Claim => 2,
        other => panic!("{EDIT_003}: the edit API does not create a {other}"),
    }
}

/// The kind an operation creates, its parent (`None` for a root) and its
/// caller id (`None` for a generated one).
fn parts(operation: &Operation) -> (ElementKind, Option<&ElementId>, Option<&ElementId>) {
    match operation {
        Operation::CreateAssuranceCasePackage(create) => {
            (AssuranceCasePackage, create.parent(), create.id())
        }
        Operation::CreateArgumentPackage(create) => (ArgumentPackage, create.parent(), create.id()),
        Operation::CreateClaim(create) => (Claim, Some(create.parent()), create.id()),
        other => panic!("{EDIT_002}: an operation this test does not know: {other:?}"),
    }
}

/// Whether `value` is an `NCName`, for the ASCII ids this file uses: a letter
/// or `_`, then letters, digits, `_`, `-` or `.`.
fn is_ascii_ncname(value: &str) -> bool {
    assert!(
        value.is_ascii(),
        "{EDIT_002}: the test classifies ASCII ids only, not {value:?}"
    );
    let mut chars = value.chars();
    chars
        .next()
        .is_some_and(|first| first.is_ascii_alphabetic() || first == '_')
        && chars.all(|c| c.is_ascii_alphanumeric() || matches!(c, '_' | '-' | '.'))
}

/// What `edit::apply` must answer for `operation` on `doc`, by this file's
/// own table and the ADR 0019 check order.
fn expected_answer(doc: &Document, operation: &Operation) -> Result<(), Code> {
    let (kind, parent, caller_id) = parts(operation);
    if let Some(parent) = parent {
        let Some(parent) = doc.get(parent) else {
            return Err(Code::CMD_TARGET_NOT_FOUND);
        };
        if !containment_table().contains(&(parent.kind(), kind)) {
            return Err(Code::CMD_INVALID_PARENT);
        }
    }
    if let Some(caller_id) = caller_id {
        if !is_ascii_ncname(caller_id.as_str()) {
            return Err(Code::ID_INVALID);
        }
        if doc.contains(caller_id) {
            return Err(Code::CMD_DUPLICATE_ID);
        }
    }
    Ok(())
}

// ---------------------------------------------------------------------------
// The starting documents
// ---------------------------------------------------------------------------

/// Built with `edit::apply` only: two more roots, depth 4, generated and
/// caller ids, and names with absent, empty and non-ASCII fields.
fn built_with_edit() -> Document {
    let row = EDIT_002;
    let mut doc = Document::new();
    let case = create(
        row,
        &mut doc,
        CreateAssuranceCasePackage::root(named("Case")).with_id("case"),
    );
    let sub = create(
        row,
        &mut doc,
        CreateAssuranceCasePackage::new(&case, LangString::default()),
    );
    let arguments = create(
        row,
        &mut doc,
        CreateArgumentPackage::new(&sub, LangString::new("Argumentation \u{00E5}")),
    );
    create(
        row,
        &mut doc,
        CreateClaim::new(&arguments, named("Generated")),
    );
    create(
        row,
        &mut doc,
        CreateClaim::new(&arguments, named("Top")).with_id("c_top"),
    );
    let nested = create(
        row,
        &mut doc,
        CreateArgumentPackage::new(&arguments, named("Nested")).with_id("nested"),
    );
    create(
        row,
        &mut doc,
        CreateClaim::new(&nested, LangString::new("")),
    );
    create(
        row,
        &mut doc,
        CreateArgumentPackage::new(&case, named("More")).with_id("case_arguments"),
    );
    create(
        row,
        &mut doc,
        CreateArgumentPackage::root(LangString::new("").with_lang("")),
    );
    create(
        row,
        &mut doc,
        CreateAssuranceCasePackage::root(named("Second")).with_id("second"),
    );
    doc
}

/// Built with the `test-support` seam: kinds and states `edit::apply` does
/// not produce. ArtifactPackage parents, interfaces and bindings, Notes,
/// missing names, a non-default declaration, ids that generation must skip
/// (`claim_1`, `argument_package_2`), and an id that is not an `NCName`
/// (`a b`), held as inserted.
fn built_with_insert() -> Document {
    let mut doc = Document::new();
    insert(&mut doc, None, ArtifactPackage, "artifacts", None);
    insert(&mut doc, Some("artifacts"), Claim, "loose_claim", None);
    insert(&mut doc, Some("loose_claim"), Note, "note_on_claim", None);
    insert(
        &mut doc,
        Some("artifacts"),
        AssuranceCasePackage,
        "case_in_artifacts",
        Some(LangString::default()),
    );
    insert(
        &mut doc,
        Some("case_in_artifacts"),
        ArgumentPackage,
        "a b",
        None,
    );
    insert(
        &mut doc,
        Some("a b"),
        Claim,
        "claim_1",
        Some(LangString::new("Hazard \u{00E4}r hanterad").with_lang("sv")),
    );
    if let Err(error) =
        doc.set_assertion_declaration(&id("claim_1"), AssertionDeclaration::NeedsSupport)
    {
        panic!("{EDIT_002}: setup could not set the declaration of claim_1: {error}");
    }
    insert(
        &mut doc,
        Some("case_in_artifacts"),
        AssuranceCasePackageBinding,
        "case_binding",
        None,
    );
    insert(
        &mut doc,
        Some("artifacts"),
        ArgumentPackageInterface,
        "arg_interface",
        None,
    );
    insert(&mut doc, Some("arg_interface"), Claim, "claim_3", None);
    insert(
        &mut doc,
        Some("artifacts"),
        ArgumentPackageBinding,
        "arg_binding",
        None,
    );
    insert(
        &mut doc,
        Some("artifacts"),
        ArtifactPackageInterface,
        "artifact_interface",
        None,
    );
    insert(&mut doc, Some("artifacts"), Note, "note_in_artifacts", None);
    insert(
        &mut doc,
        None,
        AssuranceCasePackageInterface,
        "case_interface",
        Some(named("Interface")),
    );
    insert(
        &mut doc,
        Some("case_interface"),
        Note,
        "note_on_interface",
        None,
    );
    insert(&mut doc, None, TerminologyPackage, "terms", None);
    insert(&mut doc, Some("terms"), Term, "term_1", None);
    insert(&mut doc, None, ArgumentPackage, "argument_package_2", None);
    insert(
        &mut doc,
        Some("argument_package_2"),
        Claim,
        "c_asserted",
        Some(named("Asserted")),
    );
    insert(
        &mut doc,
        Some("argument_package_2"),
        ArgumentReasoning,
        "reasoning",
        None,
    );
    insert(
        &mut doc,
        Some("argument_package_2"),
        AssertedInference,
        "inference",
        None,
    );
    doc
}

/// Reads the fixture `name` under [`FIXTURES`].
fn read_fixture(name: &str) -> Document {
    let path = Path::new(FIXTURES).join(name);
    let bytes = std::fs::read(&path)
        .unwrap_or_else(|error| panic!("{EDIT_002}: cannot read {}: {error}", path.display()));
    match xmi::read(&bytes, &ReadOptions::default()) {
        Ok(output) => output.into_document(),
        Err(error) => panic!("{EDIT_002}: setup: the fixture {name} does not read: {error}"),
    }
}

/// Every starting document, with a label for messages, in a fixed order.
fn starting_documents() -> Vec<(String, Document)> {
    let mut starts = vec![
        ("an empty document".to_owned(), Document::new()),
        (
            "a document built with edit::apply".to_owned(),
            built_with_edit(),
        ),
        (
            "a document built with insert".to_owned(),
            built_with_insert(),
        ),
    ];
    for name in READABLE_FIXTURES {
        starts.push((format!("the fixture {name}"), read_fixture(name)));
    }
    starts
}

// ---------------------------------------------------------------------------
// The operations
// ---------------------------------------------------------------------------

/// The fixed list of operations for `doc`: under every element (pre-order)
/// and under [`NOWHERE`], one create of each kind with each id form; then
/// both root creates with each id form. Fresh caller ids are `new_<n>`,
/// unique across the list.
fn operations_for(doc: &Document) -> Vec<Operation> {
    let names = names();
    // The name of the create of kind `kind` with id form `form` under the
    // `parent`-th parent (the root creates count as one more parent).
    let name = |parent: usize, form: usize, kind: usize| -> LangString {
        names[(parent + form + kind) % names.len()].clone()
    };
    let mut fresh_index = 0;
    let mut next_fresh = || {
        let fresh = format!("new_{fresh_index}");
        fresh_index += 1;
        fresh
    };
    let first_id = doc.elements().next().map(|element| element.id().clone());

    let mut parents: Vec<ElementId> = doc.elements().map(|element| element.id().clone()).collect();
    parents.push(id(NOWHERE));

    let mut operations: Vec<Operation> = Vec::new();
    for (parent_index, parent) in parents.iter().enumerate() {
        // The parent's own id is in use; under NOWHERE, the first id is.
        let duplicate = if doc.contains(parent) {
            Some(parent.clone())
        } else {
            first_id.clone()
        };
        let mut caller_ids: Vec<Option<ElementId>> =
            vec![None, Some(id(&next_fresh())), Some(id(NOT_AN_NCNAME))];
        if let Some(duplicate) = &duplicate {
            caller_ids.push(Some(duplicate.clone()));
        }
        for (form, caller_id) in caller_ids.into_iter().enumerate() {
            let with = |operation: Operation| -> Operation {
                match (operation, &caller_id) {
                    (operation, None) => operation,
                    (Operation::CreateAssuranceCasePackage(create), Some(value)) => {
                        create.with_id(value).into()
                    }
                    (Operation::CreateArgumentPackage(create), Some(value)) => {
                        create.with_id(value).into()
                    }
                    (Operation::CreateClaim(create), Some(value)) => create.with_id(value).into(),
                    (other, Some(_)) => {
                        panic!("{EDIT_002}: an operation this test does not know: {other:?}")
                    }
                }
            };
            let name = |kind: usize| name(parent_index, form, kind);
            operations.push(with(
                CreateAssuranceCasePackage::new(parent, name(0)).into(),
            ));
            operations.push(with(CreateArgumentPackage::new(parent, name(1)).into()));
            operations.push(with(CreateClaim::new(parent, name(2)).into()));
        }
    }
    let mut root_ids: Vec<Option<ElementId>> =
        vec![None, Some(id(&next_fresh())), Some(id(NOT_AN_NCNAME))];
    if let Some(first) = &first_id {
        root_ids.push(Some(first.clone()));
    }
    for (form, caller_id) in root_ids.into_iter().enumerate() {
        let case = CreateAssuranceCasePackage::root(name(parents.len(), form, 0));
        let arguments = CreateArgumentPackage::root(name(parents.len(), form, 1));
        match caller_id {
            None => {
                operations.push(case.into());
                operations.push(arguments.into());
            }
            Some(value) => {
                operations.push(case.with_id(&value).into());
                operations.push(arguments.with_id(&value).into());
            }
        }
    }
    operations
}

// ---------------------------------------------------------------------------
// The driver
// ---------------------------------------------------------------------------

/// One application of one operation: the document before (a clone taken
/// before `apply`), after, and the answer.
struct Applied<'a> {
    context: &'a str,
    operation: &'a Operation,
    before: &'a Document,
    after: &'a Document,
    answer: &'a Result<Mutation, EditError>,
}

/// Applies every operation of the set to every starting document, alone on a
/// fresh clone and then in sequence on one document, and calls `check` on
/// each application. Returns the number of applications.
fn for_every_application(mut check: impl FnMut(&Applied<'_>)) -> usize {
    let mut count = 0;
    for (label, start) in starting_documents() {
        let operations = operations_for(&start);
        for (index, operation) in operations.iter().enumerate() {
            let before = start.clone();
            let mut after = start.clone();
            let answer = edit::apply(&mut after, operation.clone());
            let context = format!("{label}, alone, operation {index} {operation:?}");
            check(&Applied {
                context: &context,
                operation,
                before: &before,
                after: &after,
                answer: &answer,
            });
            count += 1;
        }
        let mut doc = start.clone();
        for (index, operation) in operations.iter().enumerate() {
            let before = doc.clone();
            let answer = edit::apply(&mut doc, operation.clone());
            let context = format!("{label}, in sequence, operation {index} {operation:?}");
            check(&Applied {
                context: &context,
                operation,
                before: &before,
                after: &doc,
                answer: &answer,
            });
            count += 1;
        }
    }
    count
}

// ---------------------------------------------------------------------------
// LIB-EDIT-002: atomic mutations
// ---------------------------------------------------------------------------

#[test]
fn lib_edit_002_every_starting_document_is_structurally_valid() {
    for (label, start) in starting_documents() {
        assert_structurally_valid(EDIT_002, &format!("{label}, before any edit"), &start);
    }
    // The set reaches the states it claims to.
    let inserted = built_with_insert();
    for (value, kind) in [
        ("artifacts", ArtifactPackage),
        ("note_on_claim", Note),
        ("arg_interface", ArgumentPackageInterface),
        ("case_binding", AssuranceCasePackageBinding),
    ] {
        assert_eq!(
            inserted.get(value).map(SacmElement::kind),
            Some(kind),
            "{EDIT_002}: setup: the inserted document lacks the {kind} {value:?}"
        );
    }
    assert_eq!(
        inserted.get("loose_claim").and_then(SacmElement::name),
        None,
        "{EDIT_002}: setup: the inserted document has no missing name"
    );
}

#[test]
fn lib_edit_002_the_set_has_legal_and_illegal_operations_of_every_kind_with_both_id_forms() {
    // (kind, caller id given, answer) for every application.
    let mut reached: BTreeSet<(ElementKind, bool, Result<(), &'static str>)> = BTreeSet::new();
    let total = for_every_application(|applied| {
        let expected = expected_answer(applied.before, applied.operation);
        let actual = applied.answer.as_ref().map(|_| ()).map_err(EditError::code);
        assert_eq!(
            actual, expected,
            "{EDIT_002}: {}: the answer is not the one the standard's table and ADR 0019 give",
            applied.context
        );
        let (kind, _, caller_id) = parts(applied.operation);
        reached.insert((kind, caller_id.is_some(), expected.map_err(Code::as_str)));
    });
    assert!(total > 0, "{EDIT_002}: the set is empty");
    let mut required: Vec<(ElementKind, bool, Result<(), &'static str>)> = Vec::new();
    for kind in [AssuranceCasePackage, ArgumentPackage, Claim] {
        for caller in [false, true] {
            required.push((kind, caller, Ok(())));
            required.push((kind, caller, Err(Code::CMD_TARGET_NOT_FOUND.as_str())));
            required.push((kind, caller, Err(Code::CMD_INVALID_PARENT.as_str())));
        }
        required.push((kind, true, Err(Code::ID_INVALID.as_str())));
        required.push((kind, true, Err(Code::CMD_DUPLICATE_ID.as_str())));
    }
    for case in required {
        assert!(
            reached.contains(&case),
            "{EDIT_002}: the set never reaches (kind, caller id, answer) = {case:?}"
        );
    }
}

#[test]
fn lib_edit_002_every_successful_operation_leaves_the_document_structurally_valid() {
    let mut successes = 0;
    for_every_application(|applied| {
        if applied.answer.is_ok() {
            successes += 1;
            assert_structurally_valid(EDIT_002, applied.context, applied.after);
        }
    });
    assert!(
        successes > 0,
        "{EDIT_002}: no operation of the set succeeded"
    );
}

#[test]
fn lib_edit_002_every_refused_operation_leaves_the_document_equal_with_the_same_next_ids() {
    let mut refusals = 0;
    for_every_application(|applied| {
        let Err(error) = applied.answer else {
            return;
        };
        refusals += 1;
        let context = applied.context;
        assert_eq!(
            semantic_compare(applied.before, applied.after),
            [],
            "{EDIT_002}: {context}: the refusal {error:?} changed the SACM content"
        );
        assert!(
            applied.after == applied.before,
            "{EDIT_002}: {context}: after the refusal {error:?} the document is not == to before"
        );
        assert_eq!(
            next_generated_ids(EDIT_002, applied.after),
            next_generated_ids(EDIT_002, applied.before),
            "{EDIT_002}: {context}: the refusal {error:?} changed the next generated ids"
        );
    });
    assert!(
        refusals > 0,
        "{EDIT_002}: no operation of the set was refused"
    );
}

// ---------------------------------------------------------------------------
// LIB-EDIT-003: mutation results
// ---------------------------------------------------------------------------

/// The three id lists `semantic_compare(before, after)` gives: right-only
/// (right's pre-order), per-element (the ids on both sides with any
/// per-element difference, in `after`'s pre-order) and left-only (left's
/// pre-order).
struct Oracle {
    created: Vec<ElementId>,
    changed: Vec<ElementId>,
    deleted: Vec<ElementId>,
}

fn oracle(context: &str, before: &Document, after: &Document) -> Oracle {
    let mut created = Vec::new();
    let mut deleted = Vec::new();
    let mut changed = BTreeSet::new();
    for difference in semantic_compare(before, after) {
        match &difference {
            Difference::RightOnly { id, .. } => created.push(id.clone()),
            Difference::LeftOnly { id, .. } => deleted.push(id.clone()),
            Difference::Kind { id, .. }
            | Difference::Parent { id, .. }
            | Difference::Name { id, .. }
            | Difference::AssertionDeclaration { id, .. }
            | Difference::ChildOrder { id, .. } => {
                changed.insert(id.clone());
            }
            Difference::RootOrder { .. } => panic!(
                "{EDIT_003}: {context}: the edit reordered the roots, which no id list reports: {difference}"
            ),
            other => {
                panic!("{EDIT_003}: {context}: a Difference this test does not know: {other:?}")
            }
        }
    }
    let changed = after
        .elements()
        .map(|element| element.id().clone())
        .filter(|id| changed.contains(id))
        .collect();
    Oracle {
        created,
        changed,
        deleted,
    }
}

#[test]
fn lib_edit_003_the_id_lists_equal_the_semantic_comparison_of_before_and_after() {
    let mut successes = 0;
    for_every_application(|applied| {
        let Ok(mutation) = applied.answer else {
            return;
        };
        successes += 1;
        let context = applied.context;
        let expected = oracle(context, applied.before, applied.after);
        assert_eq!(
            mutation.created_ids(),
            expected.created,
            "{EDIT_003}: {context}: created_ids() is not the right-only ids"
        );
        assert_eq!(
            mutation.changed_ids(),
            expected.changed,
            "{EDIT_003}: {context}: changed_ids() is not the per-element ids"
        );
        assert_eq!(
            mutation.deleted_ids(),
            expected.deleted,
            "{EDIT_003}: {context}: deleted_ids() is not the left-only ids"
        );
    });
    assert!(
        successes > 0,
        "{EDIT_003}: no operation of the set succeeded"
    );
}

#[test]
fn lib_edit_003_a_create_creates_its_element_and_changes_and_deletes_nothing() {
    for_every_application(|applied| {
        let Ok(mutation) = applied.answer else {
            return;
        };
        let context = applied.context;
        let (kind, parent, caller_id) = parts(applied.operation);
        let own = match caller_id {
            Some(value) => value.clone(),
            None => next_generated_ids(EDIT_003, applied.before)[probe_index(kind)].clone(),
        };
        assert_eq!(
            mutation.created_ids(),
            [own],
            "{EDIT_003}: {context}: created_ids() is not the one element the create made"
        );
        assert_eq!(
            mutation.changed_ids(),
            [] as [ElementId; 0],
            "{EDIT_003}: {context}: a create reported changed ids"
        );
        assert_eq!(
            mutation.deleted_ids(),
            [] as [ElementId; 0],
            "{EDIT_003}: {context}: a create reported deleted ids"
        );
        // The parent's own features and the order of its existing children
        // are unchanged, so the comparison reports nothing for it.
        if let Some(parent) = parent {
            let about_parent: Vec<Difference> = semantic_compare(applied.before, applied.after)
                .into_iter()
                .filter(|difference| difference.element_id() == Some(parent))
                .collect();
            assert_eq!(
                about_parent,
                [],
                "{EDIT_003}: {context}: the create changed its parent {parent:?}"
            );
        }
    });
}

#[test]
fn lib_edit_003_operation_is_the_operation_given_with_a_generated_id_not_filled_in() {
    let mut generated = 0;
    for_every_application(|applied| {
        let Ok(mutation) = applied.answer else {
            return;
        };
        let context = applied.context;
        assert_eq!(
            mutation.operation(),
            applied.operation,
            "{EDIT_003}: {context}: operation() is not the operation given"
        );
        let (_, _, given_id) = parts(applied.operation);
        let (_, _, held_id) = parts(mutation.operation());
        assert_eq!(
            held_id, given_id,
            "{EDIT_003}: {context}: operation() does not hold the id as given"
        );
        if given_id.is_none() {
            generated += 1;
        }
    });
    assert!(
        generated > 0,
        "{EDIT_003}: no successful operation of the set had a generated id"
    );
}

#[test]
fn lib_edit_003_operation_name_is_the_struct_name() {
    let name = named("N");
    let cases: [(Operation, &str); 5] = [
        (
            CreateAssuranceCasePackage::root(name.clone()).into(),
            "CreateAssuranceCasePackage",
        ),
        (
            CreateAssuranceCasePackage::new("p", name.clone())
                .with_id("x")
                .into(),
            "CreateAssuranceCasePackage",
        ),
        (
            CreateArgumentPackage::root(name.clone()).into(),
            "CreateArgumentPackage",
        ),
        (
            CreateArgumentPackage::new("p", name.clone())
                .with_id("x")
                .into(),
            "CreateArgumentPackage",
        ),
        (CreateClaim::new("p", name).into(), "CreateClaim"),
    ];
    for (operation, expected) in &cases {
        assert_eq!(
            operation.operation_name(),
            *expected,
            "{EDIT_003}: operation_name() of {operation:?}"
        );
    }
}

#[test]
fn lib_edit_003_operation_name_is_the_name_of_the_operation_held_by_the_mutation() {
    let mut doc = Document::new();
    let mut names = BTreeMap::new();
    let operations: [Operation; 3] = [
        CreateAssuranceCasePackage::root(named("Case"))
            .with_id("case")
            .into(),
        CreateArgumentPackage::new("case", named("Arguments")).into(),
        CreateClaim::new("argument_package_1", named("Claim")).into(),
    ];
    for operation in operations {
        let mutation = match edit::apply(&mut doc, operation.clone()) {
            Ok(mutation) => mutation,
            Err(error) => panic!("{EDIT_003}: setup: {operation:?} was refused: {error}"),
        };
        let name: &'static str = mutation.operation().operation_name();
        names.insert(mutation.created_ids()[0].as_str().to_owned(), name);
    }
    assert_eq!(
        names,
        BTreeMap::from([
            ("argument_package_1".to_owned(), "CreateArgumentPackage"),
            ("case".to_owned(), "CreateAssuranceCasePackage"),
            ("claim_1".to_owned(), "CreateClaim"),
        ]),
        "{EDIT_003}: the operation names held by the mutations"
    );
}

fn assert_send_sync<T: Send + Sync>() {}

#[test]
fn lib_edit_003_mutation_and_operation_are_send_and_sync() {
    assert_send_sync::<Mutation>();
    assert_send_sync::<Operation>();
}

fn assert_value_type<T: Clone + Debug + PartialEq + Eq>() {}

#[test]
fn lib_edit_003_mutation_is_clone_debug_partial_eq_and_eq() {
    assert_value_type::<Mutation>();
    let apply = |name: &str| -> Mutation {
        let mut doc = Document::new();
        create(
            EDIT_003,
            &mut doc,
            CreateArgumentPackage::root(named("A")).with_id("arguments"),
        );
        match edit::apply(
            &mut doc,
            CreateClaim::new("arguments", named(name)).with_id("c1"),
        ) {
            Ok(mutation) => mutation,
            Err(error) => panic!("{EDIT_003}: setup: the claim was refused: {error}"),
        }
    };
    let first = apply("Same");
    let second = apply("Same");
    assert_eq!(
        first, second,
        "{EDIT_003}: equal operations on equal documents gave unequal mutations"
    );
    assert_eq!(first.clone(), first, "{EDIT_003}: a clone is not equal");
    // Same ids, another operation: the operation is part of equality.
    let other = apply("Other");
    assert_eq!(
        other.created_ids(),
        first.created_ids(),
        "{EDIT_003}: setup: the two mutations created different ids"
    );
    assert_ne!(
        other, first,
        "{EDIT_003}: mutations of different operations are equal"
    );
    let shown = format!("{first:?}");
    assert!(
        shown.contains("c1"),
        "{EDIT_003}: the Debug of a mutation does not show its ids: {shown}"
    );
}
