//! Tests for atomic batches of operations (LIB-EDIT-014; ADR 0032), and for
//! the revision a committed or rolled-back batch leaves (LIB-EDIT-007; ADR
//! 0031).
//!
//! - `edit::batch(&mut doc, |tx| ...)` commits exactly when the closure
//!   returns `Ok`. Each `tx.apply` answers as `edit::apply` would on the
//!   state so far, so a later operation can use an id an earlier one
//!   generated; a refusal the closure ignores changed nothing, and the rest
//!   commits.
//! - On an `Err` from the closure, a refusal passed on with `?`, or a panic,
//!   the document is `==` to before, with the same next generated id of
//!   every kind and the same revision.
//! - `edit::apply_all(&mut doc, ops)` equals `batch` over the same list with
//!   `?`; its `BatchError` names the first refusal by index, operation and
//!   `EditError`.
//! - `BatchMutation::mutations()` holds the accepted mutations in order, and
//!   its net `created_ids`, `changed_ids` and `deleted_ids` are the ids
//!   `semantic_compare(before, after)` reports right-only, per element and
//!   left-only, as for LIB-EDIT-003: created ids in creation order (when each
//!   was first created in the batch), changed ids in after's pre-order and
//!   deleted ids in before's pre-order.
//!
//! # The random sequences
//!
//! There is no `proptest` dependency, so [`Rng`] is a small deterministic
//! generator (`SplitMix64`) run from the fixed list [`SEEDS`]. For each
//! starting document and seed, [`random_run`] applies [`STEPS`] operations in
//! one batch, each drawn by [`random_operation`] from the state the batch has
//! reached (`tx.document()`): every create (generated, fresh, reused freed
//! and duplicate ids), every set and reference edit, reorders, and deletes
//! under all twelve policy combinations, often of an element the batch
//! created. Many are refused; the closure ignores the refusals. The tests
//! then replay, compare and roll back those sequences.
//!
//! Revisions are compared only for equality, never for their values.
//!
//! Not checked here:
//! - The rustdoc example that builds an `ArgumentReasoning` and an
//!   `AssertedInference` naming it in one batch: a doctest.
//! - The serde shape of `BatchMutation` and `BatchError`: `serde.rs`.

use std::collections::BTreeSet;
use std::convert::Infallible;
use std::error::Error as _;
use std::fmt::Debug;
use std::panic::{AssertUnwindSafe, catch_unwind};

use sacm::compare::{Difference, semantic_compare};
// The changes between two documents of slice 0083 (LIB-RT-005).
use sacm::compare::changes;
use sacm::edit::{
    self, AddReference, Batch, BatchError, BatchMutation, CreateArgumentGroup,
    CreateArgumentPackage, CreateArgumentReasoning, CreateArtifactGroup, CreateArtifactPackage,
    CreateArtifactReference, CreateAssertedArtifactContext, CreateAssertedArtifactSupport,
    CreateAssertedContext, CreateAssertedEvidence, CreateAssertedInference,
    CreateAssuranceCasePackage, CreateClaim, CreateDescription, CreateImplementationConstraint,
    CreateNote, CreateParticipant, CreateProperty, CreateTaggedValue, CreateTechnique,
    CrossPackageReferencePolicy, DeleteElement, EditError, Mutation, Operation,
    PackageDeletePolicy, ReferenceDeletePolicy, RemoveReference, ReorderElements,
    SetAssertionDeclaration, SetContent, SetFlag, SetGid, SetKey, SetName, SetReferences,
};
// The operations of slice 0055 (ADR 0034 amendment 1).
use sacm::edit::{
    CreateActivity, CreateArtifact, CreateEvent, CreateResource, SetDate, SetEndTime, SetLocation,
    SetStartTime, SetVersion,
};
// The operations of slice 0056 (ADR 0036).
use sacm::edit::{CopyElements, MoveElement};
use sacm::model::{
    AssertionDeclaration, Document, ElementId, ElementKind, Flag, LangString, MultiLangString,
    ReferenceEnd, SacmElement,
};
use sacm::xmi::{self, ReadOptions};

const EDIT_014: &str = "LIB-EDIT-014";
const EDIT_007: &str = "LIB-EDIT-007";
const RT_005: &str = "LIB-RT-005";

/// An id no starting document holds.
const NOWHERE: &str = "nowhere";

/// The operations of one random batch.
const STEPS: usize = 32;

/// The fixed seeds of the random sequences.
const SEEDS: [u64; 12] = [
    1,
    2,
    3,
    7,
    42,
    1_000,
    65_537,
    0x5EED,
    0xDEAD_BEEF,
    0x0123_4567_89AB_CDEF,
    0xFFFF_FFFF_FFFF_FFFF,
    20_261_115,
];

/// The twelve reference ends.
const ENDS: [ReferenceEnd; 12] = [
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
];

/// The three flags.
const FLAGS: [Flag; 3] = [Flag::IsCitation, Flag::IsAbstract, Flag::IsCounter];

/// A document with references inside one package, across packages and into
/// a nested package, an asserted relationship that is the target of another,
/// `UtilityElement`s, a dangling id (`x`) and a second `ArgumentPackage` `q`:
///
/// - `p`: Claims `c1` (with a Description and a Note; meta-claims `m1` and
///   the dangling `x`), `c2` and `m1`; the `ArgumentReasoning` `why`
///   (structure `q`); the `ArtifactReference` `r1` (referencing `c1`); the
///   `ArgumentGroup` `g1` (`c1`, `c2`); the `AssertedInference` `i1` (`c2` to
///   `c1`, reasoning `why`); the `AssertedEvidence` `e1` (`r1` to `c2`); the
///   `AssertedContext` `ctx1` (`r1`, `c2` to `i1`); and the nested
///   `ArgumentPackage` `pn` (an `argumentElement` of `p`) with the Claim `c4`
///   (meta-claim `c1`).
/// - `q`: the Claim `c3` (meta-claim `c2`) and the `AssertedInference` `i2`
///   (`c3` to `c1`).
const BATCH_START: &str = r#"<sacm:AssuranceCasePackage xmlns:sacm="http://www.omg.org/spec/SACM/20220301" xmlns:xmi="http://www.omg.org/spec/XMI/20131001" xmi:id="case">
  <name content="Case"/>
  <argumentPackage xmi:id="p">
    <name content="P"/>
    <argumentElement xmi:type="sacm:Claim" xmi:id="c1" metaClaim="m1 x">
      <name content="C1"/>
      <description xmi:id="d1"><content><value lang="en" content="One"/></content></description>
      <note xmi:id="n1"/>
    </argumentElement>
    <argumentElement xmi:type="sacm:Claim" xmi:id="c2"><name content="C2"/></argumentElement>
    <argumentElement xmi:type="sacm:Claim" xmi:id="m1"><name content="M1"/></argumentElement>
    <argumentElement xmi:type="sacm:ArgumentReasoning" xmi:id="why" structure="q"><name content="Why"/></argumentElement>
    <argumentElement xmi:type="sacm:ArtifactReference" xmi:id="r1" referencedArtifactElement="c1"><name content="R1"/></argumentElement>
    <argumentElement xmi:type="sacm:ArgumentGroup" xmi:id="g1" argumentElement="c1 c2"><name content="G1"/></argumentElement>
    <argumentElement xmi:type="sacm:AssertedInference" xmi:id="i1" source="c2" target="c1" reasoning="why"><name content="I1"/></argumentElement>
    <argumentElement xmi:type="sacm:AssertedEvidence" xmi:id="e1" source="r1" target="c2"><name content="E1"/></argumentElement>
    <argumentElement xmi:type="sacm:AssertedContext" xmi:id="ctx1" source="r1 c2" target="i1"><name content="Ctx1"/></argumentElement>
    <argumentElement xmi:type="sacm:ArgumentPackage" xmi:id="pn">
      <name content="Nested"/>
      <argumentElement xmi:type="sacm:Claim" xmi:id="c4" metaClaim="c1"><name content="C4"/></argumentElement>
    </argumentElement>
  </argumentPackage>
  <argumentPackage xmi:id="q">
    <name content="Q"/>
    <argumentElement xmi:type="sacm:Claim" xmi:id="c3" metaClaim="c2"/>
    <argumentElement xmi:type="sacm:AssertedInference" xmi:id="i2" source="c3" target="c1"/>
  </argumentPackage>
</sacm:AssuranceCasePackage>
"#;

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

fn id(value: &str) -> ElementId {
    ElementId::from(value)
}

fn named(content: &str) -> LangString {
    LangString::new(content).with_lang("en")
}

/// Two values in two languages.
fn two_values() -> MultiLangString {
    [
        named("Reviewed"),
        LangString::new("Granskad").with_lang("sv"),
    ]
    .into_iter()
    .collect()
}

fn read_text(what: &str, input: &[u8]) -> Document {
    match xmi::read(input, &ReadOptions::default()) {
        Ok(output) => output.into_document(),
        Err(error) => panic!("{EDIT_014}: setup: {what} does not read: {error}"),
    }
}

fn batch_start() -> Document {
    read_text("BATCH_START", BATCH_START.as_bytes())
}

/// Applies `operation`, which must be accepted, to `doc`.
fn applied(doc: &mut Document, operation: impl Into<Operation>) -> Mutation {
    let operation = operation.into();
    match edit::apply(doc, operation.clone()) {
        Ok(mutation) => mutation,
        Err(error) => panic!("{EDIT_014}: setup: {operation:?} was refused: {error}"),
    }
}

/// The one id `mutation` created.
fn only_created(mutation: &Mutation) -> ElementId {
    match mutation.created_ids() {
        [only] => only.clone(),
        other => panic!("{EDIT_014}: setup: {mutation:?} created {other:?}, not one element"),
    }
}

/// An ArgumentPackage `arguments` holding the Claim `goal`, built with
/// `edit::apply`, plus generated ids so that counters are not at 0.
fn gsn_start() -> Document {
    let mut doc = Document::new();
    applied(
        &mut doc,
        CreateArgumentPackage::root(named("Arguments")).with_id("arguments"),
    );
    applied(
        &mut doc,
        CreateClaim::new("arguments", named("The system is safe")).with_id("goal"),
    );
    applied(&mut doc, CreateClaim::new("arguments", named("Generated")));
    applied(
        &mut doc,
        CreateArgumentReasoning::new("arguments", named("Generated reasoning")),
    );
    doc
}

/// The worked fixture of slice 0052: `ArtifactPackage`s, a Technique with a
/// Property, Participants and an `ArtifactGroup` with members.
const ARTIFACT_FIXTURE: &str = concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/tests/data/sacm23/xmi/artifact-package-valid.sacm.xmi"
);

fn artifact_start() -> Document {
    let bytes = std::fs::read(ARTIFACT_FIXTURE).unwrap_or_else(|error| {
        panic!("{EDIT_014}: setup: cannot read {ARTIFACT_FIXTURE}: {error}")
    });
    read_text("the artifact fixture", &bytes)
}

/// The starting documents of the random sequences.
fn starting_documents() -> Vec<(&'static str, Document)> {
    vec![
        ("an empty document", Document::new()),
        ("a document built with edit::apply", gsn_start()),
        ("the batch start document", batch_start()),
        ("the artifact fixture", artifact_start()),
    ]
}

/// One create of each of the 20 kinds the edit API creates, with generated
/// ids, under a fresh root ArgumentPackage and Claim, and a fresh root
/// ArtifactPackage, applied to a clone of `doc`: the ids show the next
/// generated id of every kind.
fn next_generated_ids(doc: &Document) -> Vec<ElementId> {
    let mut probe = doc.clone();
    let name = || named("probe");
    let case = only_created(&applied(
        &mut probe,
        CreateAssuranceCasePackage::root(name()),
    ));
    let arguments = only_created(&applied(&mut probe, CreateArgumentPackage::root(name())));
    let claim = only_created(&applied(&mut probe, CreateClaim::new(&arguments, name())));
    let (p, c) = (&arguments, &claim);
    let rest: Vec<Operation> = vec![
        CreateArgumentReasoning::new(p, name()).into(),
        CreateArtifactReference::new(p, name()).into(),
        CreateArgumentGroup::new(p, name()).into(),
        CreateAssertedInference::new(p, name(), [c], c).into(),
        CreateAssertedContext::new(p, name(), [c], c).into(),
        CreateAssertedEvidence::new(p, name(), [c], c).into(),
        CreateAssertedArtifactSupport::new(p, name(), [c], c).into(),
        CreateAssertedArtifactContext::new(p, name(), [c], c).into(),
        CreateDescription::new(c, name()).into(),
        CreateImplementationConstraint::new(c, name()).into(),
        CreateNote::new(c, name()).into(),
        CreateTaggedValue::new(c, name(), name()).into(),
    ];
    let mut ids = vec![case, arguments.clone(), claim.clone()];
    for operation in rest {
        ids.push(only_created(&applied(&mut probe, operation)));
    }
    let artifacts = only_created(&applied(&mut probe, CreateArtifactPackage::root(name())));
    let a = &artifacts;
    let artifact_rest: Vec<Operation> = vec![
        CreateProperty::new(a, name()).into(),
        CreateTechnique::new(a, name()).into(),
        CreateParticipant::new(a, name()).into(),
        CreateArtifactGroup::new(a, name()).into(),
        CreateArtifact::new(a, name()).into(),
        CreateEvent::new(a, name()).into(),
        CreateResource::new(a, name()).into(),
        CreateActivity::new(a, name()).into(),
    ];
    ids.push(artifacts.clone());
    for operation in artifact_rest {
        ids.push(only_created(&applied(&mut probe, operation)));
    }
    ids
}

/// The document after `tx.apply` would refuse nothing: `doc` with every
/// operation of `operations` applied one by one, each accepted.
fn applied_one_by_one(start: &Document, operations: &[Operation]) -> (Document, Vec<Mutation>) {
    let mut doc = start.clone();
    let mutations = operations
        .iter()
        .map(|operation| applied(&mut doc, operation.clone()))
        .collect();
    (doc, mutations)
}

/// `doc` is `==` to `before`, with the same next generated ids and the same
/// revision as `before_revision` (a rolled-back batch).
fn assert_as_before<R: PartialEq + Debug>(
    context: &str,
    doc: &Document,
    before: &Document,
    before_revision: &R,
    revision: &R,
) {
    assert_eq!(
        doc, before,
        "{EDIT_014}: {context}: the document is not == to before"
    );
    assert_eq!(
        next_generated_ids(doc),
        next_generated_ids(before),
        "{EDIT_014}: {context}: the next generated ids are not those of before"
    );
    assert_eq!(
        revision, before_revision,
        "{EDIT_014}: {context}: the revision is not the one before"
    );
}

/// The ids `element` holds in `end`, in order.
fn held(element: SacmElement<'_>, end: ReferenceEnd) -> Vec<ElementId> {
    fn one(id: Option<&ElementId>) -> Vec<ElementId> {
        id.into_iter().cloned().collect()
    }
    match end {
        ReferenceEnd::CitedElement => one(element.cited_element()),
        ReferenceEnd::AbstractForm => one(element.abstract_form()),
        ReferenceEnd::MetaClaim => element.meta_claims().to_vec(),
        ReferenceEnd::Reasoning => one(element.reasoning()),
        // `sources()` and `targets()` answer for both relationship kinds
        // (ADR 0034 amendment 2): each end is its own kind's only.
        ReferenceEnd::Source if element.kind() != ElementKind::ArtifactAssetRelationship => {
            element.sources().to_vec()
        }
        ReferenceEnd::ArtifactSource
            if element.kind() == ElementKind::ArtifactAssetRelationship =>
        {
            element.sources().to_vec()
        }
        ReferenceEnd::ArtifactTarget
            if element.kind() == ElementKind::ArtifactAssetRelationship =>
        {
            element.targets().to_vec()
        }
        ReferenceEnd::Source | ReferenceEnd::ArtifactSource | ReferenceEnd::ArtifactTarget => {
            Vec::new()
        }
        ReferenceEnd::Target => one(element.target()),
        ReferenceEnd::Structure => one(element.structure()),
        ReferenceEnd::ReferencedArtifactElement => element.referenced_artifact_elements().to_vec(),
        // `group_members()` answers for both group kinds (ADR 0034): each end
        // is the members of its own group kind only.
        ReferenceEnd::GroupMember if element.kind() == ElementKind::ArgumentGroup => {
            element.group_members().to_vec()
        }
        ReferenceEnd::ArtifactGroupMember if element.kind() == ElementKind::ArtifactGroup => {
            element.group_members().to_vec()
        }
        ReferenceEnd::GroupMember | ReferenceEnd::ArtifactGroupMember => Vec::new(),
        other => panic!("{EDIT_014}: an end this test does not know: {other:?}"),
    }
}

// ---------------------------------------------------------------------------
// The oracle: semantic_compare(before, after)
// ---------------------------------------------------------------------------

/// The net ids of a batch from `before` to `after`: the right-only,
/// per-element and left-only ids of `semantic_compare(before, after)`.
/// `RootOrder` is about the document and maps to no list (ADR 0029).
///
/// The order (ADR 0032, as LIB-EDIT-003 defines it): created ids in creation
/// order, each by when it was first created in the batch, read from the
/// `created_ids` of the batch's accepted mutations; changed ids in after's
/// pre-order; deleted ids in before's pre-order.
#[derive(Debug, PartialEq, Eq)]
struct NetIds {
    created: Vec<ElementId>,
    changed: Vec<ElementId>,
    deleted: Vec<ElementId>,
}

fn net_ids<'m>(
    context: &str,
    before: &Document,
    after: &Document,
    mutations: impl IntoIterator<Item = &'m Mutation>,
) -> NetIds {
    let mut right_only = Vec::new();
    let mut deleted = Vec::new();
    let mut changed = BTreeSet::new();
    for difference in semantic_compare(before, after) {
        match &difference {
            Difference::RightOnly { id, .. } => right_only.push(id.clone()),
            Difference::LeftOnly { id, .. } => deleted.push(id.clone()),
            Difference::RootOrder { .. } => {}
            Difference::Kind { id, .. }
            | Difference::Parent { id, .. }
            | Difference::Gid { id, .. }
            | Difference::IsCitation { id, .. }
            | Difference::IsAbstract { id, .. }
            | Difference::IsCounter { id, .. }
            | Difference::CitedElement { id, .. }
            | Difference::AbstractForm { id, .. }
            | Difference::MetaClaims { id, .. }
            | Difference::Reasoning { id, .. }
            | Difference::Sources { id, .. }
            | Difference::Target { id, .. }
            | Difference::Structure { id, .. }
            | Difference::ReferencedArtifactElements { id, .. }
            | Difference::GroupMembers { id, .. }
            | Difference::ArtifactGroupMembers { id, .. }
            | Difference::ArtifactSources { id, .. }
            | Difference::ArtifactTargets { id, .. }
            | Difference::Name { id, .. }
            | Difference::NameId { id, .. }
            | Difference::Content { id, .. }
            | Difference::ContentId { id, .. }
            | Difference::ContentValueIds { id, .. }
            | Difference::Key { id, .. }
            | Difference::KeyId { id, .. }
            | Difference::KeyValueIds { id, .. }
            | Difference::AssertionDeclaration { id, .. }
            | Difference::ChildOrder { id, .. }
            // The values of slice 0055 (ADR 0034 amendment 1).
            | Difference::Version { id, .. }
            | Difference::Date { id, .. }
            | Difference::Location { id, .. }
            | Difference::LocationId { id, .. }
            | Difference::LocationValueIds { id, .. }
            | Difference::StartTime { id, .. }
            | Difference::EndTime { id, .. } => {
                changed.insert(id.clone());
            }
            other => {
                panic!("{EDIT_014}: {context}: a Difference this test does not know: {other:?}")
            }
        }
    }
    let changed = after
        .elements()
        .map(|element| element.id().clone())
        .filter(|id| changed.contains(id))
        .collect();
    // Creation order: the first creation of each right-only id. A right-only
    // id that no mutation created (a wrong Mutation) goes last, in after's
    // pre-order, so that the comparison fails on it.
    let surviving: BTreeSet<&ElementId> = right_only.iter().collect();
    let mut created: Vec<ElementId> = Vec::new();
    let creations: Vec<&ElementId> = mutations
        .into_iter()
        .flat_map(Mutation::created_ids)
        .collect();
    for id in creations.into_iter().chain(&right_only) {
        if surviving.contains(id) && !created.contains(id) {
            created.push(id.clone());
        }
    }
    NetIds {
        created,
        changed,
        deleted,
    }
}

/// The net ids of `batch`, as [`NetIds`].
fn batch_net_ids(batch: &BatchMutation) -> NetIds {
    NetIds {
        created: batch.created_ids().to_vec(),
        changed: batch.changed_ids().to_vec(),
        deleted: batch.deleted_ids().to_vec(),
    }
}

// ---------------------------------------------------------------------------
// The random sequences
// ---------------------------------------------------------------------------

/// `SplitMix64`: a small deterministic generator, the same on every platform
/// and run.
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
}

/// What a random sequence has done so far.
struct History {
    /// Prefixes the fresh caller ids, unique per seed.
    prefix: u64,
    fresh: usize,
    /// The ids created by accepted operations, in order.
    created: Vec<ElementId>,
    /// The ids deleted by accepted operations, in order.
    deleted: Vec<ElementId>,
}

/// An element id of `ids`, or [`NOWHERE`] about once in `ids.len() + 1`.
fn random_id(rng: &mut Rng, ids: &[ElementId]) -> ElementId {
    rng.pick(ids).cloned().unwrap_or_else(|| id(NOWHERE))
}

/// A caller id or none: generated (two in five), fresh, a freed id the
/// sequence deleted, or an id in use (a duplicate).
fn random_caller_id(rng: &mut Rng, ids: &[ElementId], history: &mut History) -> Option<ElementId> {
    match rng.below(5) {
        0 | 1 => None,
        2 => {
            history.fresh += 1;
            Some(id(&format!("b{}_{}", history.prefix, history.fresh)))
        }
        3 => Some(
            rng.pick(&history.deleted)
                .cloned()
                .unwrap_or_else(|| random_id(rng, ids)),
        ),
        _ => Some(random_id(rng, ids)),
    }
}

fn random_name(rng: &mut Rng) -> LangString {
    match rng.below(4) {
        0 => named("New"),
        1 => LangString::default(),
        2 => LangString::new(""),
        _ => LangString::new("S\u{e4}kerhet \u{2713} <&>").with_lang("sv-SE"),
    }
}

/// One create of an asserted relationship of a random kind, with 0 to 2
/// sources, a target, maybe a reasoning and a random `isCounter`.
fn random_relationship(
    rng: &mut Rng,
    ids: &[ElementId],
    parent: &ElementId,
    name: LangString,
    caller: Option<&ElementId>,
) -> Operation {
    let count = rng.below(3);
    let sources: Vec<ElementId> = (0..count).map(|_| random_id(rng, ids)).collect();
    let target = random_id(rng, ids);
    let reasoning = rng.flip().then(|| random_id(rng, ids));
    let is_counter = rng.flip();
    macro_rules! relationship {
        ($create:ident) => {{
            let mut create =
                $create::new(parent, name, sources, target).with_is_counter(is_counter);
            if let Some(reasoning) = &reasoning {
                create = create.with_reasoning(reasoning);
            }
            match caller {
                Some(value) => Operation::from(create.with_id(value)),
                None => Operation::from(create),
            }
        }};
    }
    match rng.below(5) {
        0 => relationship!(CreateAssertedInference),
        1 => relationship!(CreateAssertedContext),
        2 => relationship!(CreateAssertedEvidence),
        3 => relationship!(CreateAssertedArtifactSupport),
        _ => relationship!(CreateAssertedArtifactContext),
    }
}

/// A delete under a random one of the twelve policy combinations, half the
/// time of an element the sequence created.
fn random_delete(rng: &mut Rng, ids: &[ElementId], history: &History) -> Operation {
    let element = if rng.flip() {
        rng.pick(&history.created)
            .cloned()
            .unwrap_or_else(|| random_id(rng, ids))
    } else {
        random_id(rng, ids)
    };
    let reference = match rng.below(3) {
        0 => ReferenceDeletePolicy::RejectIfReferenced,
        1 => ReferenceDeletePolicy::DeleteReferencingRelationships,
        _ => ReferenceDeletePolicy::ScrubReferences,
    };
    let package = if rng.flip() {
        PackageDeletePolicy::RejectIfNonEmpty
    } else {
        PackageDeletePolicy::DeleteRecursively
    };
    let cross = if rng.flip() {
        CrossPackageReferencePolicy::RejectIfExternalReferencesExist
    } else {
        CrossPackageReferencePolicy::ApplyReferencePolicy
    };
    DeleteElement::new(element)
        .with_reference_policy(reference)
        .with_package_policy(package)
        .with_cross_package_policy(cross)
        .into()
}

/// A reference edit of a random end of `element`.
fn random_reference_edit(
    rng: &mut Rng,
    doc: &Document,
    ids: &[ElementId],
    element: &ElementId,
) -> Operation {
    let end = ENDS[rng.below(ENDS.len())];
    match rng.below(3) {
        0 => {
            let count = rng.below(3);
            if count == 0 {
                SetReferences::clear(element, end).into()
            } else {
                let list: Vec<ElementId> = (0..count).map(|_| random_id(rng, ids)).collect();
                SetReferences::new(element, end, list).into()
            }
        }
        1 => AddReference::new(element, end, random_id(rng, ids)).into(),
        _ => {
            let held_ids = doc
                .get(element)
                .map(|found| held(found, end))
                .unwrap_or_default();
            let value = if rng.flip() {
                rng.pick(&held_ids).cloned()
            } else {
                None
            };
            let value = value.unwrap_or_else(|| random_id(rng, ids));
            RemoveReference::new(element, end, value).into()
        }
    }
}

/// A reorder: of the roots (one in four), or of `element`'s children,
/// reversed, or of a random id.
fn random_reorder(
    rng: &mut Rng,
    doc: &Document,
    ids: &[ElementId],
    element: &ElementId,
) -> Operation {
    if rng.below(4) == 0 {
        let mut roots: Vec<ElementId> = doc.roots().map(|root| root.id().clone()).collect();
        roots.reverse();
        return ReorderElements::roots(roots).into();
    }
    let mut children: Vec<ElementId> = doc
        .get(element)
        .map(|found| found.children().map(|child| child.id().clone()).collect())
        .unwrap_or_default();
    children.reverse();
    if children.is_empty() || rng.below(4) == 0 {
        children = vec![random_id(rng, ids)];
    }
    ReorderElements::new(element, children).into()
}

/// One operation drawn from the full operation set, for the state `doc`.
fn random_operation(rng: &mut Rng, doc: &Document, history: &mut History) -> Operation {
    let ids: Vec<ElementId> = doc.elements().map(|element| element.id().clone()).collect();
    let target = random_id(rng, &ids);
    let other = random_id(rng, &ids);
    let name = random_name(rng);
    let caller = random_caller_id(rng, &ids, history);
    macro_rules! create {
        ($create:expr) => {
            match &caller {
                Some(value) => Operation::from($create.with_id(value)),
                None => Operation::from($create),
            }
        };
    }
    match rng.below(27) {
        0 => create!(CreateAssuranceCasePackage::new(&target, name)),
        1 => create!(CreateAssuranceCasePackage::root(name)),
        2 => create!(CreateArgumentPackage::new(&target, name)),
        3 => create!(CreateArgumentPackage::root(name)),
        4 | 5 => create!(CreateClaim::new(&target, name)),
        6 => {
            let mut reasoning = CreateArgumentReasoning::new(&target, name);
            if rng.flip() {
                reasoning = reasoning.with_structure(&other);
            }
            create!(reasoning)
        }
        7 => create!(
            CreateArtifactReference::new(&target, name).with_referenced_artifact_elements([&other])
        ),
        8 => create!(CreateArgumentGroup::new(&target, name).with_group_members([&other])),
        9 | 10 => random_relationship(rng, &ids, &target, name, caller.as_ref()),
        11 => match rng.below(10) {
            0 => create!(CreateDescription::new(&target, two_values())),
            1 => create!(CreateImplementationConstraint::new(&target, name)),
            2 => create!(CreateNote::new(&target, name)),
            3 => create!(CreateTaggedValue::new(&target, name, LangString::new("D"))),
            // The creates of slice 0052 (ADR 0034).
            4 => create!(CreateArtifactPackage::new(&target, name)),
            5 => create!(CreateArtifactPackage::root(name)),
            6 => create!(CreateProperty::new(&target, name)),
            7 => create!(CreateTechnique::new(&target, name)),
            8 => create!(CreateParticipant::new(&target, name)),
            _ => create!(CreateArtifactGroup::new(&target, name).with_group_members([&other])),
        },
        12 => SetName::new(&target, name).into(),
        13 => {
            if rng.flip() {
                SetGid::new(&target, "G-1").into()
            } else {
                SetGid::remove(&target).into()
            }
        }
        14 => SetFlag::new(&target, FLAGS[rng.below(FLAGS.len())], rng.flip()).into(),
        15 => {
            let all = AssertionDeclaration::ALL;
            let declaration = all[rng.below(all.len())];
            SetAssertionDeclaration::new(&target, declaration).into()
        }
        16 => {
            if rng.flip() {
                SetContent::new(&target, two_values()).into()
            } else {
                SetContent::remove(&target).into()
            }
        }
        17 => SetKey::new(&target, named("SIL")).into(),
        18..=20 => random_reference_edit(rng, doc, &ids, &target),
        21 => random_reorder(rng, doc, &ids, &target),
        // The creates and value edits of slice 0055 (ADR 0034 amendment 1).
        22 => match rng.below(9) {
            0 => create!(
                CreateArtifact::new(&target, name)
                    .with_version("1.2")
                    .with_date("2024-03-01")
            ),
            1 => create!(CreateEvent::new(&target, name).with_date("spring 2024")),
            2 => create!(CreateResource::new(&target, name).with_location(two_values())),
            3 => create!(
                CreateActivity::new(&target, name)
                    .with_start_time("2024-02-12")
                    .with_end_time("2024-02-01")
            ),
            4 if rng.flip() => SetVersion::new(&target, "1.3").into(),
            4 => SetVersion::remove(&target).into(),
            5 if rng.flip() => SetDate::new(&target, "2024-03-06").into(),
            5 => SetDate::remove(&target).into(),
            6 if rng.flip() => SetLocation::new(&target, two_values()).into(),
            6 => SetLocation::remove(&target).into(),
            7 if rng.flip() => SetStartTime::new(&target, "2024-02-13").into(),
            7 => SetStartTime::remove(&target).into(),
            _ if rng.flip() => SetEndTime::new(&target, "2024-02-28").into(),
            _ => SetEndTime::remove(&target).into(),
        },
        _ => random_delete(rng, &ids, history),
    }
}

/// One random batch: its starting document, every `tx.apply` call with its
/// answer, the committed `BatchMutation` and the document after.
struct Run {
    context: String,
    start: Document,
    calls: Vec<(Operation, Result<Mutation, BatchError>)>,
    outcome: BatchMutation,
    after: Document,
}

impl Run {
    fn operations(&self) -> Vec<Operation> {
        self.calls
            .iter()
            .map(|(operation, _)| operation.clone())
            .collect()
    }

    fn accepted(&self) -> Vec<&Mutation> {
        self.calls
            .iter()
            .filter_map(|(_, answer)| answer.as_ref().ok())
            .collect()
    }
}

/// Applies [`STEPS`] random operations to a clone of `start` in one batch
/// whose closure ignores every refusal and returns `Ok`.
fn random_run(label: &str, start: &Document, seed: u64) -> Run {
    let mut rng = Rng(seed);
    let mut history = History {
        prefix: seed % 1_000,
        fresh: 0,
        created: Vec::new(),
        deleted: Vec::new(),
    };
    let mut calls = Vec::new();
    let mut doc = start.clone();
    let outcome = edit::batch(&mut doc, |tx| {
        for _ in 0..STEPS {
            let operation = random_operation(&mut rng, tx.document(), &mut history);
            let answer = tx.apply(operation.clone());
            if let Ok(mutation) = &answer {
                history.created.extend_from_slice(mutation.created_ids());
                history.deleted.extend_from_slice(mutation.deleted_ids());
            }
            calls.push((operation, answer));
        }
        Ok::<(), Infallible>(())
    });
    let outcome = match outcome {
        Ok(outcome) => outcome,
        Err(never) => match never {},
    };
    Run {
        context: format!("{label}, seed {seed}"),
        start: start.clone(),
        calls,
        outcome,
        after: doc,
    }
}

/// Every random run: each starting document with each seed.
fn random_runs() -> Vec<Run> {
    let mut runs = Vec::new();
    for (label, start) in starting_documents() {
        for seed in SEEDS {
            runs.push(random_run(label, &start, seed));
        }
    }
    runs
}

/// The random runs reach what they are meant to test: refusals, accepted
/// deletes that cascade or scrub, reorders, and elements created and then
/// deleted in one batch.
fn assert_runs_cover(runs: &[Run]) {
    let mut refused = 0;
    let mut deletes = 0;
    let mut cascades = 0;
    let mut scrubs = 0;
    let mut reorders = 0;
    let mut created_then_deleted = 0;
    let mut net_deleted = 0;
    let mut net_created = 0;
    for run in runs {
        let mut created_here: BTreeSet<&ElementId> = BTreeSet::new();
        for (operation, answer) in &run.calls {
            let Ok(mutation) = answer else {
                refused += 1;
                continue;
            };
            created_here.extend(mutation.created_ids());
            match operation {
                Operation::DeleteElement(_) => {
                    deletes += 1;
                    if mutation.deleted_ids().len() > 1 {
                        cascades += 1;
                    }
                    if !mutation.changed_ids().is_empty() {
                        scrubs += 1;
                    }
                    if mutation
                        .deleted_ids()
                        .iter()
                        .any(|deleted| created_here.contains(deleted))
                    {
                        created_then_deleted += 1;
                    }
                }
                Operation::ReorderElements(_) => reorders += 1,
                _ => {}
            }
        }
        net_deleted += usize::from(!run.outcome.deleted_ids().is_empty());
        net_created += usize::from(!run.outcome.created_ids().is_empty());
    }
    for (what, count) in [
        ("refused operations", refused),
        ("accepted deletes", deletes),
        ("deletes of more than one element", cascades),
        ("deletes that changed a surviving holder", scrubs),
        ("accepted reorders", reorders),
        (
            "deletes of an element created in the same batch",
            created_then_deleted,
        ),
        ("batches with net deleted ids", net_deleted),
        ("batches with net created ids", net_created),
    ] {
        assert!(
            count > 0,
            "{EDIT_014}: setup: the random sequences have no {what}"
        );
    }
}

// ---------------------------------------------------------------------------
// Committing: per-operation answers and later use of generated ids
// ---------------------------------------------------------------------------

#[test]
fn lib_edit_014_over_random_sequences_each_answer_equals_applying_the_operations_one_by_one() {
    for run in random_runs() {
        let context = &run.context;
        let mut doc = run.start.clone();
        for (index, (operation, answer)) in run.calls.iter().enumerate() {
            let alone = edit::apply(&mut doc, operation.clone());
            match (answer, &alone) {
                (Ok(in_batch), Ok(one_by_one)) => assert_eq!(
                    in_batch, one_by_one,
                    "{EDIT_014}: {context}: call {index} {operation:?}: the Mutation differs \
                     from applying one by one"
                ),
                (Err(in_batch), Err(one_by_one)) => {
                    assert_eq!(
                        in_batch.index(),
                        index,
                        "{EDIT_014}: {context}: call {index}: the BatchError index is not the \
                         number of earlier tx.apply calls"
                    );
                    assert_eq!(
                        in_batch.operation(),
                        operation,
                        "{EDIT_014}: {context}: call {index}: the BatchError operation is not \
                         the one refused"
                    );
                    assert_eq!(
                        in_batch.error(),
                        one_by_one,
                        "{EDIT_014}: {context}: call {index} {operation:?}: the EditError \
                         differs from applying one by one"
                    );
                    assert_eq!(
                        in_batch.code(),
                        one_by_one.code(),
                        "{EDIT_014}: {context}: call {index}: code() is not the EditError's"
                    );
                }
                (in_batch, one_by_one) => panic!(
                    "{EDIT_014}: {context}: call {index} {operation:?}: the batch answered \
                     {in_batch:?}, one by one {one_by_one:?}"
                ),
            }
        }
        assert_eq!(
            run.after, doc,
            "{EDIT_014}: {context}: the committed batch is not == to applying one by one"
        );
        assert_eq!(
            next_generated_ids(&run.after),
            next_generated_ids(&doc),
            "{EDIT_014}: {context}: the committed batch generates other next ids than \
             applying one by one"
        );
        let mutations: Vec<&Mutation> = run.outcome.mutations().iter().collect();
        assert_eq!(
            mutations,
            run.accepted(),
            "{EDIT_014}: {context}: mutations() is not every accepted Mutation in order"
        );
    }
}

#[test]
fn lib_edit_014_a_later_operation_uses_an_id_an_earlier_one_generated() {
    let mut doc = gsn_start();
    let before = doc.clone();
    let mut answers: Vec<Mutation> = Vec::new();
    let result = edit::batch(&mut doc, |tx: &mut Batch<'_>| -> Result<(), BatchError> {
        let strategy = tx.apply(CreateArgumentReasoning::new(
            "arguments",
            named("Argue over each hazard"),
        ))?;
        let reasoning = only_created(&strategy);
        let sub = tx.apply(CreateClaim::new(
            "arguments",
            named("Hazard H1 is mitigated"),
        ))?;
        let sub_claim = only_created(&sub);
        let inference = tx.apply(
            CreateAssertedInference::new("arguments", named("Inference"), [&sub_claim], "goal")
                .with_reasoning(&reasoning),
        )?;
        answers.extend([strategy, sub, inference]);
        Ok(())
    });
    let batch = match result {
        Ok(batch) => batch,
        Err(error) => panic!("{EDIT_014}: the batch was refused: {error}"),
    };
    let [strategy, sub, inference] = answers.as_slice() else {
        panic!("{EDIT_014}: setup: the closure did not record three mutations");
    };
    let (reasoning, sub_claim, inference_id) = (
        only_created(strategy),
        only_created(sub),
        only_created(inference),
    );
    let Some(found) = doc.get(&inference_id) else {
        panic!("{EDIT_014}: the committed document has no {inference_id:?}");
    };
    assert_eq!(
        found.reasoning(),
        Some(&reasoning),
        "{EDIT_014}: the AssertedInference does not name the generated ArgumentReasoning"
    );
    assert_eq!(
        found.sources(),
        std::slice::from_ref(&sub_claim),
        "{EDIT_014}: the AssertedInference does not name the generated Claim"
    );
    assert_eq!(
        batch.mutations(),
        answers.as_slice(),
        "{EDIT_014}: mutations() is not the Mutations tx.apply returned, in order"
    );
    let operations: Vec<Operation> = answers
        .iter()
        .map(|mutation| mutation.operation().clone())
        .collect();
    let (alone, one_by_one) = applied_one_by_one(&before, &operations);
    assert_eq!(
        one_by_one, answers,
        "{EDIT_014}: the Mutations differ from applying the operations one by one"
    );
    assert_eq!(
        alone, doc,
        "{EDIT_014}: the committed document differs from applying the operations one by one"
    );
    assert_eq!(
        batch.created_ids(),
        [reasoning, sub_claim, inference_id],
        "{EDIT_014}: created_ids() is not the three new elements in creation order"
    );
    assert!(
        batch.changed_ids().is_empty() && batch.deleted_ids().is_empty(),
        "{EDIT_014}: three creates changed or deleted {:?} {:?}",
        batch.changed_ids(),
        batch.deleted_ids()
    );
}

#[test]
fn lib_edit_014_batch_document_shows_the_state_so_far_and_check_answers_on_it() {
    let mut doc = batch_start();
    let before = doc.clone();
    let mut seen = None;
    let result = edit::batch(&mut doc, |tx| -> Result<(), BatchError> {
        let claim = only_created(&tx.apply(CreateClaim::new("p", named("Seen")))?);
        let follow = Operation::from(AddReference::new("c2", ReferenceEnd::MetaClaim, &claim));
        let contains = tx.document().contains(&claim);
        let checked = edit::check(tx.document(), &follow);
        let checked_before = edit::check(&before, &follow);
        let answer = tx
            .apply(follow)
            .map(drop)
            .map_err(|error| error.error().clone());
        seen = Some((
            contains,
            checked,
            checked_before,
            answer,
            tx.document().clone(),
        ));
        Ok(())
    });
    assert!(
        result.is_ok(),
        "{EDIT_014}: setup: the batch was refused: {result:?}"
    );
    let Some((contains, checked, checked_before, answer, last)) = seen else {
        panic!("{EDIT_014}: setup: the closure did not run");
    };
    assert!(
        contains,
        "{EDIT_014}: tx.document() does not hold the Claim created earlier in the batch"
    );
    assert_eq!(
        checked,
        Ok(()),
        "{EDIT_014}: check on tx.document() refuses a reference to the Claim created earlier"
    );
    assert!(
        checked_before.is_err(),
        "{EDIT_014}: setup: check on the document before the batch accepts the reference"
    );
    assert_eq!(
        answer, checked,
        "{EDIT_014}: tx.apply answers otherwise than check on tx.document()"
    );
    assert_eq!(
        last, doc,
        "{EDIT_014}: tx.document() at the end of the closure is not the committed document"
    );
}

#[test]
fn lib_edit_014_a_refusal_the_closure_ignores_changes_nothing_and_the_rest_commits() {
    let mut doc = batch_start();
    let before = doc.clone();
    let first = Operation::from(CreateClaim::new("p", named("Before the refusal")));
    let refused = Operation::from(DeleteElement::new("c1"));
    let last = Operation::from(SetName::new("c2", named("After the refusal")));
    let mut around = None;
    let result = edit::batch(&mut doc, |tx| -> Result<(), BatchError> {
        tx.apply(first.clone())?;
        let state = tx.document().clone();
        let answer = tx.apply(refused.clone());
        let after_refusal = tx.document().clone();
        tx.apply(last.clone())?;
        around = Some((state, answer, after_refusal));
        Ok(())
    });
    let batch = match result {
        Ok(batch) => batch,
        Err(error) => panic!("{EDIT_014}: the batch was refused: {error}"),
    };
    let Some((state, answer, after_refusal)) = around else {
        panic!("{EDIT_014}: setup: the closure did not run");
    };
    let Err(error) = answer else {
        panic!("{EDIT_014}: setup: deleting the referenced c1 was accepted: {answer:?}");
    };
    assert_eq!(
        error.index(),
        1,
        "{EDIT_014}: the refused call is not index 1"
    );
    assert_eq!(
        after_refusal, state,
        "{EDIT_014}: the refused operation changed tx.document()"
    );
    let (alone, mutations) = applied_one_by_one(&before, &[first, last]);
    assert_eq!(
        doc, alone,
        "{EDIT_014}: the batch did not commit the operations around the ignored refusal"
    );
    assert_eq!(
        batch.mutations(),
        mutations.as_slice(),
        "{EDIT_014}: mutations() is not the two accepted Mutations"
    );
}

#[test]
fn lib_edit_014_batch_error_index_counts_every_tx_apply_call_from_0() {
    let mut doc = batch_start();
    let before = doc.clone();
    let before_revision = doc.revision();
    let operations: [Operation; 4] = [
        CreateClaim::new("p", named("Zero")).into(),
        SetName::new(NOWHERE, named("One")).into(),
        SetName::new("c2", named("Two")).into(),
        CreateClaim::new(NOWHERE, named("Three")).into(),
    ];
    let mut swallowed = None;
    let result = edit::batch(&mut doc, |tx| -> Result<(), BatchError> {
        tx.apply(operations[0].clone())?;
        swallowed = tx.apply(operations[1].clone()).err();
        tx.apply(operations[2].clone())?;
        tx.apply(operations[3].clone())?;
        Ok(())
    });
    let Some(swallowed) = swallowed else {
        panic!("{EDIT_014}: setup: SetName of {NOWHERE:?} was accepted");
    };
    assert_eq!(
        swallowed.index(),
        1,
        "{EDIT_014}: the ignored refusal of the second call is not index 1"
    );
    let Err(error) = result else {
        panic!("{EDIT_014}: the batch committed past a refusal passed on with ?: {result:?}");
    };
    assert_eq!(
        error.index(),
        3,
        "{EDIT_014}: the refusal of the fourth call is not index 3 (an ignored refusal counts)"
    );
    assert_eq!(
        error.operation(),
        &operations[3],
        "{EDIT_014}: the BatchError does not hold the refused operation"
    );
    let rolled_back = doc.revision();
    assert_as_before(
        "a refusal passed on with ?",
        &doc,
        &before,
        &before_revision,
        &rolled_back,
    );
}

// ---------------------------------------------------------------------------
// Rolling back
// ---------------------------------------------------------------------------

/// A closure's own error.
#[derive(Debug, PartialEq, Eq)]
struct Abandoned {
    reason: &'static str,
    accepted: usize,
}

/// Operations that `batch_start` accepts one by one: generated and caller
/// ids, a scrubbing delete across packages, a reorder, a recursive delete
/// and a Note.
fn accepted_sequence() -> Vec<Operation> {
    vec![
        CreateClaim::new("p", named("New claim")).into(),
        CreateClaim::new("q", named("Caller id"))
            .with_id("kept")
            .into(),
        SetName::new("c2", named("C2 renamed")).into(),
        DeleteElement::new("c1")
            .with_reference_policy(ReferenceDeletePolicy::ScrubReferences)
            .with_cross_package_policy(CrossPackageReferencePolicy::ApplyReferencePolicy)
            .into(),
        ReorderElements::new("p", ["m1", "c2"]).into(),
        DeleteElement::new("pn")
            .with_package_policy(PackageDeletePolicy::DeleteRecursively)
            .into(),
        CreateNote::new("c2", named("Reviewed")).into(),
    ]
}

/// How many of `operations` `start` accepts, applied one by one (at least
/// one).
fn accepted_one_by_one(start: &Document, operations: &[Operation]) -> usize {
    let mut doc = start.clone();
    let accepted = operations
        .iter()
        .filter(|operation| edit::apply(&mut doc, (*operation).clone()).is_ok())
        .count();
    assert!(
        accepted > 0,
        "{EDIT_014}: setup: no operation of the sequence is accepted"
    );
    accepted
}

#[test]
fn lib_edit_014_an_err_from_the_closure_is_returned_unchanged_and_the_document_is_as_before() {
    let mut doc = batch_start();
    let before = doc.clone();
    let before_revision = doc.revision();
    let operations = accepted_sequence();
    let count = accepted_one_by_one(&before, &operations);
    let result = edit::batch(&mut doc, |tx| {
        let mut accepted = 0;
        for operation in operations {
            if tx.apply(operation).is_ok() {
                accepted += 1;
            }
        }
        Err(Abandoned {
            reason: "the user cancelled",
            accepted,
        })
    });
    assert_eq!(
        result,
        Err(Abandoned {
            reason: "the user cancelled",
            accepted: count,
        }),
        "{EDIT_014}: the closure's error is not returned unchanged"
    );
    let revision = doc.revision();
    assert_as_before(
        "an Err after the accepted operations",
        &doc,
        &before,
        &before_revision,
        &revision,
    );
}

/// An application error that a refusal converts into, so that the closure
/// can use `?` on `tx.apply`.
#[derive(Debug, PartialEq)]
enum AppError {
    Refused(BatchError),
}

impl From<BatchError> for AppError {
    fn from(error: BatchError) -> Self {
        Self::Refused(error)
    }
}

#[test]
fn lib_edit_014_a_refusal_passed_on_with_the_question_mark_rolls_back_the_batch() {
    let mut doc = batch_start();
    let before = doc.clone();
    let before_revision = doc.revision();
    let operations: [Operation; 3] = [
        CreateClaim::new("p", named("Kept until the refusal")).into(),
        DeleteElement::new("c1").into(),
        SetName::new("c2", named("Never applied")).into(),
    ];
    let result = edit::batch(&mut doc, |tx| -> Result<(), AppError> {
        for operation in &operations {
            tx.apply(operation.clone())?;
        }
        Ok(())
    });
    let mut alone = before.clone();
    applied(&mut alone, operations[0].clone());
    let Err(expected) = edit::apply(&mut alone, operations[1].clone()) else {
        panic!("{EDIT_014}: setup: deleting the referenced c1 was accepted");
    };
    match result {
        Err(AppError::Refused(error)) => {
            assert_eq!(error.index(), 1, "{EDIT_014}: the refusal is not index 1");
            assert_eq!(
                error.operation(),
                &operations[1],
                "{EDIT_014}: the BatchError does not hold the refused operation"
            );
            assert_eq!(
                error.error(),
                &expected,
                "{EDIT_014}: the BatchError's EditError is not the one-by-one refusal"
            );
        }
        other => panic!("{EDIT_014}: the batch answered {other:?}, not the refusal"),
    }
    let revision = doc.revision();
    assert_as_before(
        "a refusal passed on with ? into the closure's own error",
        &doc,
        &before,
        &before_revision,
        &revision,
    );
}

#[test]
fn lib_edit_014_an_error_type_without_from_batch_error_needs_no_conversion() {
    let mut doc = batch_start();
    let before = doc.clone();
    let before_revision = doc.revision();
    let result = edit::batch(&mut doc, |tx| -> Result<(), &'static str> {
        if tx.apply(CreateClaim::new("p", named("Applied"))).is_err() {
            return Err("setup: the create was refused");
        }
        if tx.apply(DeleteElement::new("c1")).is_err() {
            return Err("refused");
        }
        Ok(())
    });
    assert_eq!(
        result,
        Err("refused"),
        "{EDIT_014}: the closure's &str error is not returned unchanged"
    );
    let revision = doc.revision();
    assert_as_before(
        "an Err of a type without From<BatchError>",
        &doc,
        &before,
        &before_revision,
        &revision,
    );
}

#[test]
fn lib_edit_014_a_panic_in_the_closure_leaves_the_document_as_before() {
    let mut doc = batch_start();
    let before = doc.clone();
    let before_revision = doc.revision();
    let operations = accepted_sequence();
    let outcome = catch_unwind(AssertUnwindSafe(|| {
        edit::batch(&mut doc, |tx| -> Result<(), BatchError> {
            for operation in operations {
                let _ = tx.apply(operation);
            }
            panic!("{EDIT_014}: the closure panics after its operations, as the test intends");
        })
    }));
    assert!(
        outcome.is_err(),
        "{EDIT_014}: setup: the panic in the closure did not reach catch_unwind"
    );
    let revision = doc.revision();
    assert_as_before(
        "a panic after the accepted operations",
        &doc,
        &before,
        &before_revision,
        &revision,
    );
}

#[test]
fn lib_edit_014_over_random_sequences_an_err_from_the_closure_rolls_everything_back() {
    for run in random_runs() {
        let context = &run.context;
        let mut doc = run.start.clone();
        let before_revision = doc.revision();
        let operations = run.operations();
        let result = edit::batch(&mut doc, |tx| {
            let mut accepted = 0;
            for operation in operations {
                if tx.apply(operation).is_ok() {
                    accepted += 1;
                }
            }
            Err(Abandoned {
                reason: "rolled back",
                accepted,
            })
        });
        assert_eq!(
            result,
            Err(Abandoned {
                reason: "rolled back",
                accepted: run.accepted().len(),
            }),
            "{EDIT_014}: {context}: the closure's error is not returned unchanged"
        );
        let revision = doc.revision();
        assert_as_before(context, &doc, &run.start, &before_revision, &revision);
    }
}

// ---------------------------------------------------------------------------
// apply_all
// ---------------------------------------------------------------------------

#[test]
fn lib_edit_014_over_random_sequences_apply_all_equals_batch_over_the_same_list() {
    for run in random_runs() {
        let context = &run.context;
        let operations = run.operations();

        // The whole list, refusals included: apply_all stops at the first.
        let mut via_all = run.start.clone();
        let before_revision = via_all.revision();
        let all = edit::apply_all(&mut via_all, operations.clone());
        let mut via_batch = run.start.clone();
        let batched = edit::batch(&mut via_batch, |tx| {
            for operation in &operations {
                tx.apply(operation.clone())?;
            }
            Ok::<(), BatchError>(())
        });
        assert_eq!(
            all, batched,
            "{EDIT_014}: {context}: apply_all answers otherwise than batch over the same list"
        );
        assert_eq!(
            via_all, via_batch,
            "{EDIT_014}: {context}: apply_all leaves another document than batch"
        );
        let first_refusal = run.calls.iter().position(|(_, answer)| answer.is_err());
        match (&all, first_refusal) {
            (Err(error), Some(k)) => {
                let Some((operation, Err(in_batch))) = run.calls.get(k) else {
                    panic!("{EDIT_014}: {context}: setup: call {k} is not a refusal");
                };
                assert_eq!(
                    error.index(),
                    k,
                    "{EDIT_014}: {context}: apply_all does not name the first refusal"
                );
                assert_eq!(
                    error.operation(),
                    operation,
                    "{EDIT_014}: {context}: apply_all's BatchError holds another operation"
                );
                assert_eq!(
                    error.error(),
                    in_batch.error(),
                    "{EDIT_014}: {context}: apply_all's EditError is not the refusal's"
                );
                assert_eq!(
                    error.code(),
                    in_batch.error().code(),
                    "{EDIT_014}: {context}: code() is not the EditError's"
                );
                let revision = via_all.revision();
                assert_as_before(context, &via_all, &run.start, &before_revision, &revision);
            }
            (Ok(outcome), None) => assert_eq!(
                outcome, &run.outcome,
                "{EDIT_014}: {context}: apply_all of a list it accepts differs from the batch"
            ),
            (answer, first) => panic!(
                "{EDIT_014}: {context}: apply_all answered {answer:?}, but the first refusal \
                 of the batch is {first:?}"
            ),
        }

        // Only the accepted operations: apply_all commits them all, exactly
        // as the batch that ignored the refusals did.
        let accepted: Vec<Operation> = run
            .accepted()
            .iter()
            .map(|mutation| mutation.operation().clone())
            .collect();
        let mut doc = run.start.clone();
        let answer = edit::apply_all(&mut doc, accepted);
        assert_eq!(
            answer.as_ref(),
            Ok(&run.outcome),
            "{EDIT_014}: {context}: apply_all of the accepted operations differs from the batch \
             that ignored the refusals"
        );
        assert_eq!(
            doc, run.after,
            "{EDIT_014}: {context}: apply_all of the accepted operations leaves another document"
        );
    }
}

#[test]
fn lib_edit_014_apply_all_names_the_first_refusal_by_index_operation_and_edit_error() {
    let mut doc = batch_start();
    let before = doc.clone();
    let before_revision = doc.revision();
    let operations: Vec<Operation> = vec![
        CreateClaim::new("p", named("Zero")).into(),
        SetName::new("c2", named("One")).into(),
        DeleteElement::new("c1").into(),
        CreateNote::new("c2", named("Three")).into(),
        SetName::new(NOWHERE, named("Four")).into(),
    ];
    let mut alone = before.clone();
    applied(&mut alone, operations[0].clone());
    applied(&mut alone, operations[1].clone());
    let Err(expected) = edit::apply(&mut alone, operations[2].clone()) else {
        panic!("{EDIT_014}: setup: deleting the referenced c1 was accepted");
    };
    let result = edit::apply_all(&mut doc, operations.clone());
    let Err(error) = result else {
        panic!("{EDIT_014}: apply_all committed past a refusal: {result:?}");
    };
    assert_eq!(
        error.index(),
        2,
        "{EDIT_014}: the BatchError does not name the third operation"
    );
    assert_eq!(
        error.operation(),
        &operations[2],
        "{EDIT_014}: the BatchError does not hold the refused operation"
    );
    assert_eq!(
        error.error(),
        &expected,
        "{EDIT_014}: the BatchError's EditError is not the refusal applying one by one gives"
    );
    assert_eq!(
        error.code(),
        expected.code(),
        "{EDIT_014}: code() is not the EditError's code"
    );
    let source = error
        .source()
        .and_then(|source| source.downcast_ref::<EditError>());
    assert_eq!(
        source,
        Some(&expected),
        "{EDIT_014}: source() is not the EditError"
    );
    let revision = doc.revision();
    assert_as_before(
        "apply_all refused at index 2",
        &doc,
        &before,
        &before_revision,
        &revision,
    );
}

#[test]
fn lib_edit_014_batch_error_display_names_the_index_the_operation_and_the_refusal() {
    let mut doc = batch_start();
    let operations: [Operation; 2] = [
        CreateClaim::new("p", named("Zero")).into(),
        SetName::new(NOWHERE, named("One")).into(),
    ];
    let result = edit::apply_all(&mut doc, operations);
    let Err(error) = result else {
        panic!("{EDIT_014}: setup: apply_all accepted a SetName of {NOWHERE:?}: {result:?}");
    };
    let shown = error.to_string();
    assert_eq!(
        shown,
        "operation 1 (SetName) of the batch was refused: the document has no element \
         \"nowhere\" to edit",
        "{EDIT_014}: BatchError's Display does not name the index, the operation and the refusal"
    );
    assert_eq!(
        shown,
        format!(
            "operation {} ({}) of the batch was refused: {}",
            error.index(),
            error.operation().operation_name(),
            error.error()
        ),
        "{EDIT_014}: BatchError's Display does not end with the EditError's Display"
    );
    assert!(
        shown.starts_with(char::is_lowercase)
            && !shown.ends_with(['.', '!', '?'])
            && !shown.contains('\n'),
        "{EDIT_014}: BatchError's Display is not one lowercase line without a trailing \
         punctuation mark: {shown:?}"
    );
}

#[test]
fn lib_edit_014_apply_all_takes_any_list_of_operation_structs() {
    let mut doc = batch_start();
    let names = [
        SetName::new("c1", named("First")),
        SetName::new("c2", named("Second")),
    ];
    let result = edit::apply_all(&mut doc, names.clone());
    let batch = match result {
        Ok(batch) => batch,
        Err(error) => panic!("{EDIT_014}: apply_all of two SetName was refused: {error}"),
    };
    let operations: Vec<Operation> = names.into_iter().map(Operation::from).collect();
    let (alone, mutations) = applied_one_by_one(&batch_start(), &operations);
    assert_eq!(
        batch.mutations(),
        mutations.as_slice(),
        "{EDIT_014}: apply_all of SetName structs differs from applying them one by one"
    );
    assert_eq!(
        doc, alone,
        "{EDIT_014}: apply_all of SetName structs leaves another document"
    );
}

// ---------------------------------------------------------------------------
// Net ids
// ---------------------------------------------------------------------------

#[test]
fn lib_edit_014_over_random_sequences_the_net_ids_equal_the_semantic_comparison() {
    let runs = random_runs();
    assert_runs_cover(&runs);
    for run in &runs {
        let context = &run.context;
        let expected = net_ids(context, &run.start, &run.after, run.accepted());
        assert_eq!(
            batch_net_ids(&run.outcome),
            expected,
            "{EDIT_014}: {context}: the net ids are not the right-only (in creation order), \
             per-element (in pre-order after) and left-only (in pre-order before) ids of \
             semantic_compare(before, after)"
        );
    }
}

/// `compare::changes(start, after)` of each random batch lists the batch's
/// net ids: the changed and deleted ids exactly, the created ids as a set,
/// and as a list when the batch created them in `after`'s pre-order (a
/// batch lists them in creation order); `document_changed()` is whether
/// `semantic_compare` reports a root order, the one document-level
/// difference a batch can make.
#[test]
fn lib_rt_005_over_random_sequences_changes_equal_the_batch_s_net_ids() {
    let runs = random_runs();
    assert_runs_cover(&runs);
    let set = |ids: &[ElementId]| ids.iter().cloned().collect::<BTreeSet<_>>();
    let mut created_in_pre_order = 0;
    let mut created_out_of_pre_order = 0;
    let mut root_orders = 0;
    for run in &runs {
        let context = &run.context;
        let found = changes(&run.start, &run.after);
        let batch = &run.outcome;
        assert_eq!(
            found.changed_ids(),
            batch.changed_ids(),
            "{RT_005}: {context}: changed_ids() is not the batch's"
        );
        assert_eq!(
            found.deleted_ids(),
            batch.deleted_ids(),
            "{RT_005}: {context}: deleted_ids() is not the batch's"
        );
        assert_eq!(
            set(found.created_ids()),
            set(batch.created_ids()),
            "{RT_005}: {context}: created_ids() is not the batch's as a set"
        );
        let pre_order: Vec<&ElementId> = run
            .after
            .elements()
            .map(SacmElement::id)
            .filter(|id| batch.created_ids().contains(id))
            .collect();
        if pre_order.iter().copied().eq(batch.created_ids()) {
            assert_eq!(
                found.created_ids(),
                batch.created_ids(),
                "{RT_005}: {context}: the batch created in pre-order, and created_ids() is \
                 not its list"
            );
            created_in_pre_order += usize::from(!batch.created_ids().is_empty());
        } else {
            created_out_of_pre_order += 1;
        }
        let root_order = semantic_compare(&run.start, &run.after)
            .iter()
            .any(|difference| matches!(difference, Difference::RootOrder { .. }));
        root_orders += usize::from(root_order);
        assert_eq!(
            found.document_changed(),
            root_order,
            "{RT_005}: {context}: document_changed() is not whether the roots were reordered"
        );
        assert_eq!(
            found.is_empty(),
            run.start == run.after,
            "{RT_005}: {context}: is_empty() is not start == after"
        );
    }
    for (what, count) in [
        ("batches that created in pre-order", created_in_pre_order),
        (
            "batches that created out of pre-order",
            created_out_of_pre_order,
        ),
        ("batches that reordered the roots", root_orders),
    ] {
        assert!(
            count > 0,
            "{RT_005}: setup: the random sequences have no {what}"
        );
    }
}

#[test]
fn lib_edit_014_created_ids_are_in_creation_order_not_document_order() {
    let mut doc = batch_start();
    let before = doc.clone();
    let mut order = Vec::new();
    let result = edit::batch(&mut doc, |tx| -> Result<(), BatchError> {
        let first = only_created(&tx.apply(CreateClaim::new("q", named("First")))?);
        let second = only_created(&tx.apply(CreateClaim::new("p", named("Second")))?);
        let third = only_created(&tx.apply(CreateClaim::new("p", named("Third")))?);
        // p precedes q, and the third now precedes the second: the
        // pre-order is third, second, first.
        tx.apply(ReorderElements::new("p", [&third, &second]))?;
        order = vec![first, second, third];
        Ok(())
    });
    let batch = match result {
        Ok(batch) => batch,
        Err(error) => panic!("{EDIT_014}: the batch was refused: {error}"),
    };
    let pre_order: Vec<&ElementId> = doc
        .elements()
        .map(SacmElement::id)
        .filter(|id| order.contains(id))
        .collect();
    assert_ne!(
        pre_order,
        order.iter().collect::<Vec<_>>(),
        "{EDIT_014}: setup: the creation order is also the document order"
    );
    assert_eq!(
        batch.created_ids(),
        order.as_slice(),
        "{EDIT_014}: created_ids() is not in creation order"
    );
    assert_eq!(
        batch_net_ids(&batch),
        net_ids("creation order", &before, &doc, batch.mutations()),
        "{EDIT_014}: the net ids are not the semantic comparison"
    );
}

#[test]
fn lib_edit_014_an_element_created_and_deleted_in_the_batch_is_in_neither_list() {
    let mut doc = batch_start();
    let before = doc.clone();
    let mut temporary = None;
    let result = edit::batch(&mut doc, |tx| -> Result<(), BatchError> {
        let claim = only_created(&tx.apply(CreateClaim::new("p", named("Temporary")))?);
        tx.apply(CreateNote::new(&claim, named("Goes with it")))?;
        tx.apply(DeleteElement::new(&claim))?;
        tx.apply(SetName::new("c2", named("Renamed")))?;
        temporary = Some(claim);
        Ok(())
    });
    let batch = match result {
        Ok(batch) => batch,
        Err(error) => panic!("{EDIT_014}: the batch was refused: {error}"),
    };
    let Some(temporary) = temporary else {
        panic!("{EDIT_014}: setup: the closure did not run");
    };
    assert_eq!(
        batch.mutations().len(),
        4,
        "{EDIT_014}: mutations() does not hold the four accepted operations"
    );
    for (what, ids) in [
        ("created_ids", batch.created_ids()),
        ("changed_ids", batch.changed_ids()),
        ("deleted_ids", batch.deleted_ids()),
    ] {
        assert!(
            !ids.contains(&temporary),
            "{EDIT_014}: the Claim created and deleted in the batch is in {what}: {ids:?}"
        );
    }
    assert_eq!(
        batch_net_ids(&batch),
        NetIds {
            created: Vec::new(),
            changed: vec![id("c2")],
            deleted: Vec::new(),
        },
        "{EDIT_014}: the net ids of create, delete and rename are not just the renamed c2"
    );
    assert_eq!(
        batch_net_ids(&batch),
        net_ids("create then delete", &before, &doc, batch.mutations()),
        "{EDIT_014}: the net ids are not the semantic comparison"
    );
}

#[test]
fn lib_edit_014_an_element_deleted_and_created_again_with_its_id_is_neither_created_nor_deleted() {
    let mut doc = batch_start();
    let before = doc.clone();
    let result = edit::batch(&mut doc, |tx| -> Result<(), BatchError> {
        tx.apply(
            DeleteElement::new("m1").with_reference_policy(ReferenceDeletePolicy::ScrubReferences),
        )?;
        tx.apply(CreateClaim::new("p", LangString::new("M1")).with_id("m1"))?;
        Ok(())
    });
    let batch = match result {
        Ok(batch) => batch,
        Err(error) => panic!("{EDIT_014}: the batch was refused: {error}"),
    };
    let m1 = id("m1");
    assert!(
        !batch.created_ids().contains(&m1) && !batch.deleted_ids().contains(&m1),
        "{EDIT_014}: m1, in the document before and after, is reported created {:?} or \
         deleted {:?}",
        batch.created_ids(),
        batch.deleted_ids()
    );
    assert_eq!(
        batch_net_ids(&batch),
        net_ids("delete then create again", &before, &doc, batch.mutations()),
        "{EDIT_014}: the net ids are not the semantic comparison"
    );
}

// ---------------------------------------------------------------------------
// Moves and copies in a batch (LIB-EDIT-017, LIB-EDIT-018; ADR 0036 D4)
// ---------------------------------------------------------------------------

/// The worked document of slice 0056, as in `move_element.rs`: the
/// AssuranceCasePackage root `case` holding the ArgumentPackage `p` (the
/// Claims `c1`, `c2` with the `metaClaim` `c1`, and `c3`), the
/// ArgumentPackage `q` (the Claim `d1`) and the ArtifactPackage `e`.
const MOVE_START: &str = r#"<sacm:AssuranceCasePackage xmlns:sacm="http://www.omg.org/spec/SACM/20220301" xmlns:xmi="http://www.omg.org/spec/XMI/20131001" xmi:id="case">
  <name content="Case"/>
  <argumentPackage xmi:id="p">
    <name content="P"/>
    <argumentElement xmi:type="sacm:Claim" xmi:id="c1"><name content="C1"/></argumentElement>
    <argumentElement xmi:type="sacm:Claim" xmi:id="c2" metaClaim="c1"><name content="C2"/></argumentElement>
    <argumentElement xmi:type="sacm:Claim" xmi:id="c3"><name content="C3"/></argumentElement>
  </argumentPackage>
  <argumentPackage xmi:id="q">
    <name content="Q"/>
    <argumentElement xmi:type="sacm:Claim" xmi:id="d1"><name content="D1"/></argumentElement>
  </argumentPackage>
  <artifactPackage xmi:id="e"><name content="E"/></artifactPackage>
</sacm:AssuranceCasePackage>
"#;

#[test]
fn lib_edit_017_a_batch_that_moves_an_element_away_and_back_reports_the_parent_whose_order_changed()
{
    const EDIT_017: &str = "LIB-EDIT-017";
    let mut doc = read_text("MOVE_START", MOVE_START.as_bytes());
    let before = doc.clone();
    let result = edit::batch(&mut doc, |tx| -> Result<(), BatchError> {
        let away = tx.apply(MoveElement::new("c1", "q"))?;
        assert_eq!(
            away.changed_ids(),
            [id("c1")],
            "{EDIT_017}: setup: the move away changes c1"
        );
        let back = tx.apply(MoveElement::new("c1", "p").with_position(2))?;
        assert_eq!(
            back.changed_ids(),
            [id("c1")],
            "{EDIT_017}: setup: the move back changes c1"
        );
        Ok(())
    });
    let batch = match result {
        Ok(batch) => batch,
        Err(error) => panic!("{EDIT_017}: the batch was refused: {error}"),
    };
    let children: Vec<&str> = doc
        .get("p")
        .map(|p| p.children().map(|child| child.id().as_str()).collect())
        .unwrap_or_default();
    assert_eq!(
        children,
        ["c2", "c3", "c1"],
        "{EDIT_017}: c1 is back in p, last"
    );
    // c1 has its old parent, so it is not changed; p's child order is.
    assert_eq!(
        batch_net_ids(&batch),
        NetIds {
            created: Vec::new(),
            changed: vec![id("p")],
            deleted: Vec::new(),
        },
        "{EDIT_017}: the net ids of a move away and back are not the parent whose order changed"
    );
    assert_eq!(
        batch_net_ids(&batch),
        net_ids("a move away and back", &before, &doc, batch.mutations()),
        "{EDIT_017}: the net ids are not the semantic comparison"
    );
    // Control: away and back to its old place changes nothing.
    let mut same = before.clone();
    let operations: [Operation; 2] = [
        MoveElement::new("c1", "q").into(),
        MoveElement::new("c1", "p").with_position(0).into(),
    ];
    let batch = match edit::apply_all(&mut same, operations) {
        Ok(batch) => batch,
        Err(error) => panic!("{EDIT_017}: the batch was refused: {error}"),
    };
    assert_eq!(
        batch_net_ids(&batch),
        NetIds {
            created: Vec::new(),
            changed: Vec::new(),
            deleted: Vec::new(),
        },
        "{EDIT_017}: a move away and back to the old place reports a change"
    );
    assert!(same == before, "{EDIT_017}: the document is not as before");
}

#[test]
fn lib_edit_018_a_copy_and_an_edit_of_its_copy_commit_as_one_batch() {
    const EDIT_018: &str = "LIB-EDIT-018";
    let start = read_text("MOVE_START", MOVE_START.as_bytes());

    // p copied into case within one document, then a rename of the copy of
    // c1 by the id the copy generated, and a rename of the original c3.
    let mut doc = start.clone();
    let mut renamed = None;
    let result = edit::batch(&mut doc, |tx| -> Result<(), BatchError> {
        let copy = match CopyElements::new(tx.document(), ["p"], "case") {
            Ok(copy) => copy,
            Err(error) => panic!("{EDIT_018}: setup: the copy was refused: {error}"),
        };
        let copied = tx.apply(copy)?;
        let copy_of_c1 = copied.created_ids()[1].clone();
        tx.apply(SetName::new(&copy_of_c1, named("The copy, renamed")))?;
        tx.apply(SetName::new("c3", named("The original, renamed")))?;
        renamed = Some(copy_of_c1);
        Ok(())
    });
    let batch = match result {
        Ok(batch) => batch,
        Err(error) => panic!("{EDIT_018}: the batch was refused: {error}"),
    };
    let Some(renamed) = renamed else {
        panic!("{EDIT_018}: setup: the closure did not run");
    };
    assert_eq!(renamed, id("claim_1"), "{EDIT_018}: the copy of c1");
    assert_eq!(
        doc.get("claim_1").and_then(SacmElement::name),
        Some(&named("The copy, renamed")),
        "{EDIT_018}: the copy is renamed"
    );
    assert_eq!(
        doc.get("c1").and_then(SacmElement::name),
        Some(&LangString::new("C1")),
        "{EDIT_018}: the original is not renamed"
    );
    assert_eq!(
        batch_net_ids(&batch),
        NetIds {
            created: vec![
                id("argument_package_1"),
                id("claim_1"),
                id("claim_2"),
                id("claim_3")
            ],
            changed: vec![id("c3")],
            deleted: Vec::new(),
        },
        "{EDIT_018}: a renamed copy is created, not changed; the renamed original is changed"
    );
    assert_eq!(
        batch_net_ids(&batch),
        net_ids("a copy and a rename", &start, &doc, batch.mutations()),
        "{EDIT_018}: the net ids are not the semantic comparison"
    );

    // A refusal after the copy rolls the copy back.
    let mut rolled = start.clone();
    let revision = rolled.revision();
    let copy = match CopyElements::new(&start, ["c1"], "q") {
        Ok(copy) => copy,
        Err(error) => panic!("{EDIT_018}: setup: the copy was refused: {error}"),
    };
    let operations: [Operation; 2] = [
        copy.into(),
        SetName::new("nowhere", named("Refused")).into(),
    ];
    let refused = edit::apply_all(&mut rolled, operations);
    assert!(
        matches!(&refused, Err(error) if error.index() == 1),
        "{EDIT_018}: the batch is {refused:?}, not refused at its second operation"
    );
    assert_as_before(
        "a copy rolled back",
        &rolled,
        &start,
        &revision,
        &rolled.revision(),
    );
}

// ---------------------------------------------------------------------------
// The types
// ---------------------------------------------------------------------------

fn assert_value_type<T: Clone + Debug + PartialEq + Eq + Send + Sync>() {}

fn assert_error<T: std::error::Error + Send + Sync + 'static>() {}

#[test]
fn lib_edit_014_batch_mutation_and_batch_error_are_send_sync_value_types() {
    assert_value_type::<BatchMutation>();
    assert_value_type::<BatchError>();
    assert_error::<BatchError>();
}

// ---------------------------------------------------------------------------
// Revisions (LIB-EDIT-007)
// ---------------------------------------------------------------------------

#[test]
fn lib_edit_007_a_committed_batch_gives_a_new_revision_even_when_it_changes_nothing() {
    let mut doc = batch_start();
    let revision = doc.revision();
    let empty = edit::batch(&mut doc, |_tx| Ok::<(), Infallible>(()));
    assert!(
        empty.is_ok(),
        "{EDIT_007}: setup: an empty batch was refused"
    );
    let after_empty = doc.revision();
    assert_ne!(
        after_empty, revision,
        "{EDIT_007}: an empty committed batch kept the revision"
    );

    let refused_only = edit::batch(&mut doc, |tx| {
        let _ = tx.apply(SetName::new(NOWHERE, named("Refused")));
        Ok::<(), Infallible>(())
    });
    let committed = match refused_only {
        Ok(batch) => batch,
        Err(never) => match never {},
    };
    assert!(
        committed.mutations().is_empty(),
        "{EDIT_007}: setup: the refused SetName was committed"
    );
    let after_refused = doc.revision();
    assert_ne!(
        after_refused, after_empty,
        "{EDIT_007}: a committed batch whose only operation was refused kept the revision"
    );

    let none: Vec<Operation> = Vec::new();
    let all = edit::apply_all(&mut doc, none);
    assert!(
        all.is_ok(),
        "{EDIT_007}: setup: apply_all of no operations was refused"
    );
    assert_ne!(
        doc.revision(),
        after_refused,
        "{EDIT_007}: apply_all of no operations kept the revision"
    );
}

#[test]
fn lib_edit_007_a_rolled_back_batch_and_a_refused_apply_all_keep_the_revision() {
    let mut doc = batch_start();
    let revision = doc.revision();
    let rolled_back = edit::batch(&mut doc, |tx| -> Result<(), &'static str> {
        tx.apply(CreateClaim::new("p", named("Rolled back")))
            .map_err(|_| "setup: the create was refused")?;
        Err("rolled back")
    });
    assert_eq!(
        rolled_back,
        Err("rolled back"),
        "{EDIT_007}: setup: the batch did not roll back"
    );
    assert_eq!(
        doc.revision(),
        revision,
        "{EDIT_007}: a rolled-back batch changed the revision"
    );
    let refused = edit::apply_all(
        &mut doc,
        [
            Operation::from(CreateClaim::new("p", named("Kept"))),
            Operation::from(SetName::new(NOWHERE, named("No"))),
        ],
    );
    assert!(
        refused.is_err(),
        "{EDIT_007}: setup: apply_all accepted a list with a refusal"
    );
    assert_eq!(
        doc.revision(),
        revision,
        "{EDIT_007}: a refused apply_all changed the revision"
    );
}
