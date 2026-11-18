//! Tests for atomic edits and edit results (LIB-EDIT-002, LIB-EDIT-003;
//! ADR 0019 and its amendments 1 and 2; ADR 0020; ADR 0029), and the
//! properties the set operations of LIB-EDIT-008, LIB-EDIT-009, LIB-EDIT-010
//! and LIB-EDIT-012 must keep over the same set.
//!
//! - LIB-EDIT-002: every `edit::apply` either succeeds and leaves the
//!   document structurally valid, or refuses and leaves it `==` to a clone
//!   taken before, generating the same next ids.
//! - LIB-EDIT-003: a successful `edit::apply` returns a `Mutation` holding
//!   the operation as given and the ids created, changed and deleted, which
//!   equal what `semantic_compare(before, after)` reports right-only,
//!   per-element and left-only.
//! - LIB-EDIT-013 (ADR 0019 amendment 3): `edit::check(&doc, &op)` equals
//!   `edit::apply(&mut doc.clone(), op.clone()).map(drop)`, the same
//!   `EditError` included, and leaves the document `==` to before with the
//!   same next generated id of every kind. It runs over the set, in which
//!   every `Operation` variant is accepted and refused, the creates with both
//!   id forms. An operation that
//!   fails two checks gets the first, from both.
//! - LIB-EDIT-004 (ADR 0031): `edit::preview(&doc, &op)` answers as `apply`
//!   on a clone, its mutation included, and as `check`; it leaves the
//!   document `==`, with the same revision and next generated ids; and the
//!   dangling references of a delete are those of [`delete_plan`].
//! - LIB-EDIT-005 and LIB-EDIT-006 (ADR 0031): every `DeleteElement` of the
//!   set answers, deletes and changes what [`delete_plan`] gives, and leaves
//!   every surviving end its old list minus the deleted ids.
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
//! from each readable XMI fixture; built with the `test-support`
//! `Document::insert`, with ArtifactPackage parents, interfaces and bindings,
//! Notes, missing names, a set declaration, an id that is not an `NCName`, a
//! gid and references; and [`EDIT_START`], read from XMI, with every
//! argumentation kind, ids on a name, a `content` and a `key` and their
//! values, a dangling id, wrongly typed ids and a relationship without a
//! source.
//!
//! The operations are, for every element of the starting document and for an
//! id it does not hold ("nowhere"):
//! - the creates: one of each of the 20 kinds under it with a generated id,
//!   a fresh caller id, an id that is not an `NCName` and the parent's own
//!   (duplicate) id, the referencing ones naming ids of the document; with a
//!   generated id, each create with an end refused for no source, a
//!   white-space id, a missing id or a wrongly typed id; and both root
//!   creates with each id form (and the ArtifactPackage root create);
//! - the attribute edits (LIB-EDIT-008): a name, the current name, a gid, the
//!   current gid, gid removal, each flag true and false, a declaration and the
//!   current declaration;
//! - the content edits (LIB-EDIT-012): a two-value content, the current
//!   content, content removal, a key and the current key;
//! - the reference edits (LIB-EDIT-009), for each of the ten ends: `[]`, a
//!   missing id, `a b`, a well-typed id, two ids, a wrongly typed id, the ids
//!   held and the id of a name, `content`, `key` or `value`; an add of each of
//!   those and of a held id; a remove of a held id, of a well-typed id and of
//!   a missing one;
//! - the reorders (LIB-EDIT-010): of its children reversed, in their current
//!   order, empty, with a repeated child, a grandchild, a missing id and the
//!   id of a name; and the same for the roots;
//! - last, the deletes (LIB-EDIT-005, LIB-EDIT-006): of "nowhere", of the
//!   id of a name, and of every element in reverse pre-order (so that in
//!   sequence an element's contents go before it), each under the 12 policy
//!   combinations, from the strictest.
//!
//! [`DELETE_START`] is the starting document for the deletes: sources,
//! targets, reasonings, group members, `metaClaim`, `citedElement` and
//! `abstractForm` targets, a `structure`, `referencedArtifactElement`s, a
//! relationship that is the target and one that is the source of another
//! relationship, nested and sibling packages, Notes, a root holding only a
//! Note, and dangling ids read from the file. [`built_with_insert`] adds
//! references into a package interface, and [`EDIT_START`] more dangling
//! and wrongly typed ids.
//!
//! So the set holds legal operations and illegal ones of every check.
//!
//! # Structural validity
//!
//! [`assert_structurally_valid`] checks the properties of LIB-EDIT-002
//! through the public views only:
//! - S1: ids are unique, `get` round-trips each, and
//!   `elements().count() == element_count()`;
//! - S2: parent and child links agree both ways, and `roots()` holds exactly
//!   the parentless elements;
//! - S3: every root is a package, and every child's kind is legal under its
//!   parent's kind, by this file's own containment table (§8.6, §9.2, §10.4,
//!   §11.4, §12.2, §12.6; ADR 0014), not the crate's;
//! - S4: every Assertion (a Claim or one of the five asserted relationships)
//!   has a declaration;
//! - S7: every `xmi:id` in the document, of an element, a name, a `content`,
//!   a `key` or a `value`, is unique.
//!
//! [`assert_edit_kept_structure`] checks, for every successful operation,
//! what it must keep of the document before it:
//! - S5: every `ModelElement` that had a name still has one, and every
//!   `TaggedValue` that had a key still has one;
//! - S6: every (owner, end, id) that is dangling or wrongly typed after the
//!   operation was so before (D-VAL-LAYER);
//! - S8: no surviving asserted relationship loses its last source or its
//!   target;
//! - S9: only a `DeleteElement` removes elements.
//!
//! What each operation must answer is written in [`expected_answer`] from
//! this file's own tables (which kinds have a name, a declaration,
//! `isCounter`, a `content` and a `key`; each end's owners, declared type and
//! bounds) and the check orders of ADR 0019, ADR 0029 and ADR 0031, not from
//! the crate's tables. What a delete deletes, changes and leaves dangling is
//! written in [`delete_plan`] from ADR 0031.
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
//! - That `check` and `apply` share one crate-private `plan`, and that
//!   `commit` returns no `Result` and builds no `EditError` (LIB-EDIT-013
//!   item 4), and the example in the rustdoc of `check` (item 5): by review.

use std::collections::{BTreeMap, BTreeSet};
use std::fmt::Debug;
use std::path::Path;
use std::sync::OnceLock;

use sacm::compare::{Difference, semantic_compare};
use sacm::diagnostic::Code;
use sacm::edit::CrossPackageReferencePolicy::{
    ApplyReferencePolicy, RejectIfExternalReferencesExist,
};
use sacm::edit::PackageDeletePolicy::{DeleteRecursively, RejectIfNonEmpty};
use sacm::edit::ReferenceDeletePolicy::{
    DeleteReferencingRelationships, RejectIfReferenced, ScrubReferences,
};
use sacm::edit::{
    self, AddReference, CreateArgumentGroup, CreateArgumentPackage, CreateArgumentReasoning,
    CreateArtifactGroup, CreateArtifactPackage, CreateArtifactReference,
    CreateAssertedArtifactContext, CreateAssertedArtifactSupport, CreateAssertedContext,
    CreateAssertedEvidence, CreateAssertedInference, CreateAssuranceCasePackage, CreateClaim,
    CreateDescription, CreateImplementationConstraint, CreateNote, CreateParticipant,
    CreateProperty, CreateTaggedValue, CreateTechnique, CrossPackageReferencePolicy, DeleteElement,
    EditError, Mutation, Operation, PackageDeletePolicy, ReferenceDeletePolicy, RemoveReference,
    ReorderElements, SetAssertionDeclaration, SetContent, SetFlag, SetGid, SetKey, SetName,
    SetReferences,
};
// The operations of slice 0055 (ADR 0034 amendment 1).
use sacm::edit::{
    CreateActivity, CreateArtifact, CreateEvent, CreateResource, SetDate, SetEndTime, SetLocation,
    SetStartTime, SetVersion,
};
// The operations of slice 0056 (ADR 0036).
use sacm::edit::{CopyElements, MoveElement};
// The creates of slice 0066 (SACM23-ARG-014, -015, SACM23-ART-012, -013; ADR 0039).
use sacm::edit::{
    CreateArgumentPackageBinding, CreateArgumentPackageInterface, CreateArtifactPackageBinding,
    CreateArtifactPackageInterface,
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
    AssertionDeclaration, Document, ElementId, ElementKind, Flag, LangString, MultiLangString,
    ReferenceEnd, SacmElement,
};
use sacm::xmi::{self, ReadOptions};

const EDIT_002: &str = "LIB-EDIT-002";
const EDIT_003: &str = "LIB-EDIT-003";
const EDIT_004: &str = "LIB-EDIT-004";
const EDIT_005: &str = "LIB-EDIT-005";
const EDIT_006: &str = "LIB-EDIT-006";
const EDIT_008: &str = "LIB-EDIT-008";
const EDIT_009: &str = "LIB-EDIT-009";
const EDIT_010: &str = "LIB-EDIT-010";
const EDIT_012: &str = "LIB-EDIT-012";
const EDIT_013: &str = "LIB-EDIT-013";
const EDIT_017: &str = "LIB-EDIT-017";
const EDIT_018: &str = "LIB-EDIT-018";
const API_014: &str = "LIB-API-014";

/// The directory of the XMI fixtures.
const FIXTURES: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/tests/data/sacm23/xmi");

/// The fixtures `xmi::read` reads in this release, relative to [`FIXTURES`],
/// with the three of slice 0066: its two worked examples and
/// `golden/nested.golden.sacm.xmi`, whose `ArgumentPackageInterface` it
/// reads (SACM23-ARG-014, SACM23-ART-012; LIB-EDIT-019). The files under
/// `invalid/` are refused by design.
const READABLE_FIXTURES: [&str; 14] = [
    "argument-package-interfaces-valid.sacm.xmi",
    "artifact-package-interfaces-valid.sacm.xmi",
    "golden/nested.golden.sacm.xmi",
    "argument-package-root-valid.sacm.xmi",
    "artifact-assets-valid.sacm.xmi",
    "artifact-package-valid.sacm.xmi",
    "assurance-case-package-root-valid.sacm.xmi",
    "name-third-party-form-valid.sacm.xmi",
    "wrapped-roots-valid.sacm.xmi",
    "golden/artifact-package.golden.sacm.xmi",
    "golden/named.golden.sacm.xmi",
    "golden/needs-support-claim.golden.sacm.xmi",
    "golden/several-roots.golden.sacm.xmi",
    "golden/single-root.golden.sacm.xmi",
];

/// The parent id that no starting document holds.
const NOWHERE: &str = "nowhere";

/// The caller id that is not an `NCName`: it starts with a digit.
const NOT_AN_NCNAME: &str = "1bad";

/// The referenced id that holds white space. `built_with_insert` has an
/// element with it.
const WITH_SPACE: &str = "a b";

/// A starting document read from XMI, for the set operations: every
/// argumentation kind; the root's gid and its name's `xmi:id`; a Description
/// whose `content` and values have ids and a `TaggedValue` whose `key` and value
/// have them; a Claim without a name (`c3`) and a grandchild of the root; the
/// dangling id `x` in `c1`'s `metaClaim` and `i1`'s `source`; the wrongly
/// typed `structure` `c2`, `referencedArtifactElement` `n1` and group member
/// `case`; and the `AssertedArtifactContext` `ac1`, read without a source.
const EDIT_START: &str = r#"<sacm:AssuranceCasePackage xmlns:sacm="http://www.omg.org/spec/SACM/20220301" xmlns:xmi="http://www.omg.org/spec/XMI/20131001" xmi:id="case" gid="g-case">
  <name xmi:id="case_name" lang="en" content="Case"/>
  <argumentPackage xmi:id="p">
    <name content="P"/>
    <argumentElement xmi:type="sacm:Claim" xmi:id="c1" isCitation="true" citedElement="c2" metaClaim="m1 x">
      <name content="C1"/>
      <description xmi:id="d1">
        <content xmi:id="d1_content">
          <value xmi:id="d1_en" lang="en" content="One"/>
          <value xmi:id="d1_sv" lang="sv" content="Ett"/>
        </content>
      </description>
      <note xmi:id="n1"/>
      <taggedValue xmi:id="t1">
        <key xmi:id="t1_key"><value xmi:id="t1_key_en" lang="en" content="ASIL"/></key>
        <content><value content="D"/></content>
      </taggedValue>
    </argumentElement>
    <argumentElement xmi:type="sacm:Claim" xmi:id="c2" assertionDeclaration="needsSupport"><name content="C2"/></argumentElement>
    <argumentElement xmi:type="sacm:Claim" xmi:id="m1"><name content="M1"/></argumentElement>
    <argumentElement xmi:type="sacm:ArgumentReasoning" xmi:id="why" structure="c2"><name content="Why"/></argumentElement>
    <argumentElement xmi:type="sacm:ArtifactReference" xmi:id="r1" referencedArtifactElement="c1 n1"><name content="R1"/></argumentElement>
    <argumentElement xmi:type="sacm:ArgumentGroup" xmi:id="g1" argumentElement="c1 case"><name content="G1"/></argumentElement>
    <argumentElement xmi:type="sacm:AssertedInference" xmi:id="i1" source="c2 x" target="c1" reasoning="why"><name content="I1"/></argumentElement>
    <argumentElement xmi:type="sacm:AssertedEvidence" xmi:id="e1" isCounter="true" source="r1" target="c1"><name content="E1"/></argumentElement>
    <argumentElement xmi:type="sacm:AssertedContext" xmi:id="ctx1" source="r1" target="c2"><name content="Ctx1"/></argumentElement>
    <argumentElement xmi:type="sacm:AssertedArtifactSupport" xmi:id="s1" source="r1" target="r1"><name content="S1"/></argumentElement>
    <argumentElement xmi:type="sacm:AssertedArtifactContext" xmi:id="ac1" target="c1"><name content="Ac1"/></argumentElement>
  </argumentPackage>
  <argumentPackage xmi:id="q">
    <name content="Q"/>
    <argumentElement xmi:type="sacm:Claim" xmi:id="c3"/>
  </argumentPackage>
</sacm:AssuranceCasePackage>
"#;

/// A starting document read from XMI, for the deletes (ADR 0031). In `dp`:
/// `goal`; `sub_a`, with a Note and the `abstractForm` `goal`; `sub_b`, with
/// the `metaClaim`s `goal` and the dangling `ghost`; `strategy`, whose
/// `structure` is the nested package `dnested`; `inf_ab` from both subs to
/// `goal` with the reasoning `strategy` and a Description; `inf_a` from
/// `sub_a` alone; `ctx_on_inf`, whose target is the relationship `inf_a`;
/// `inf_on_ctx`, whose source is the relationship `ctx_on_inf`;
/// `evidence_ref`, referencing `sub_a`, `far` (in the sibling package) and
/// `ghost`; and `group`, holding `sub_a`, `inf_a` and `ghost`. In `dnested`:
/// `deep`, citing `goal`, and `inf_deep` from `deep` to `sub_b`. In the
/// sibling package `dother`: `far`, citing `sub_b` with the `metaClaim`
/// `goal`, `far_ev` from `evidence_ref` to `far`, and `far_inf` from `far` to
/// `sub_b`. A second root, `dloose`, holds only a Note.
const DELETE_START: &str = r#"<xmi:XMI xmlns:sacm="http://www.omg.org/spec/SACM/20220301" xmlns:xmi="http://www.omg.org/spec/XMI/20131001">
  <sacm:AssuranceCasePackage xmi:id="dcase">
    <name content="Delete case"/>
    <argumentPackage xmi:id="dp">
      <name content="DP"/>
      <argumentElement xmi:type="sacm:Claim" xmi:id="goal"><name content="Goal"/></argumentElement>
      <argumentElement xmi:type="sacm:Claim" xmi:id="sub_a" abstractForm="goal"><name content="A"/><note xmi:id="sub_a_note"/></argumentElement>
      <argumentElement xmi:type="sacm:Claim" xmi:id="sub_b" metaClaim="goal ghost"><name content="B"/></argumentElement>
      <argumentElement xmi:type="sacm:ArgumentReasoning" xmi:id="strategy" structure="dnested"><name content="Strategy"/></argumentElement>
      <argumentElement xmi:type="sacm:AssertedInference" xmi:id="inf_ab" source="sub_a sub_b" target="goal" reasoning="strategy">
        <name content="Iab"/>
        <description xmi:id="inf_ab_d"><content><value content="Both"/></content></description>
      </argumentElement>
      <argumentElement xmi:type="sacm:AssertedInference" xmi:id="inf_a" source="sub_a" target="goal"><name content="Ia"/></argumentElement>
      <argumentElement xmi:type="sacm:AssertedContext" xmi:id="ctx_on_inf" source="evidence_ref" target="inf_a"><name content="Ctx"/></argumentElement>
      <argumentElement xmi:type="sacm:AssertedInference" xmi:id="inf_on_ctx" source="ctx_on_inf" target="sub_b"><name content="Ictx"/></argumentElement>
      <argumentElement xmi:type="sacm:ArtifactReference" xmi:id="evidence_ref" referencedArtifactElement="sub_a far ghost"><name content="E"/></argumentElement>
      <argumentElement xmi:type="sacm:ArgumentGroup" xmi:id="group" argumentElement="sub_a inf_a ghost"><name content="G"/></argumentElement>
      <argumentElement xmi:type="sacm:ArgumentPackage" xmi:id="dnested">
        <name content="Nested"/>
        <argumentElement xmi:type="sacm:Claim" xmi:id="deep" citedElement="goal"><name content="Deep"/></argumentElement>
        <argumentElement xmi:type="sacm:AssertedInference" xmi:id="inf_deep" source="deep" target="sub_b"><name content="Id"/></argumentElement>
      </argumentElement>
    </argumentPackage>
    <argumentPackage xmi:id="dother">
      <name content="Other"/>
      <argumentElement xmi:type="sacm:Claim" xmi:id="far" citedElement="sub_b" metaClaim="goal"><name content="Far"/></argumentElement>
      <argumentElement xmi:type="sacm:AssertedEvidence" xmi:id="far_ev" source="evidence_ref" target="far"><name content="Fe"/></argumentElement>
      <argumentElement xmi:type="sacm:AssertedInference" xmi:id="far_inf" source="far" target="sub_b"><name content="Fi"/></argumentElement>
    </argumentPackage>
  </sacm:AssuranceCasePackage>
  <sacm:ArgumentPackage xmi:id="dloose"><name content="Loose"/><note xmi:id="dloose_note"/></sacm:ArgumentPackage>
</xmi:XMI>
"#;

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

/// Every kind: the 34 `ModelElement`s and the four `UtilityElement`s, all
/// `SACMElement`s (§8.2).
const ALL_KINDS: [ElementKind; 38] = [
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
    Description,
    ImplementationConstraint,
    Note,
    TaggedValue,
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
/// `assertionDeclaration` (§11.10, `[1]`) and a `metaClaim` end.
const ASSERTIONS: [ElementKind; 6] = [
    Claim,
    AssertedInference,
    AssertedEvidence,
    AssertedContext,
    AssertedArtifactSupport,
    AssertedArtifactContext,
];

/// The five `AssertedRelationship` kinds (§11.13-§11.18): what has
/// `isCounter`, `reasoning`, `source` and `target`.
const RELATIONSHIPS: [ElementKind; 5] = [
    AssertedInference,
    AssertedEvidence,
    AssertedContext,
    AssertedArtifactSupport,
    AssertedArtifactContext,
];

/// The eight `ArgumentAsset` kinds (§11.7: `ArtifactReference`,
/// `ArgumentReasoning` and the `Assertion`s): what a `source` and a `target`
/// admit.
const ARGUMENT_ASSETS: [ElementKind; 8] = [
    Claim,
    ArgumentReasoning,
    ArtifactReference,
    AssertedInference,
    AssertedEvidence,
    AssertedContext,
    AssertedArtifactSupport,
    AssertedArtifactContext,
];

/// Every legal (parent kind, child kind) pair, from the 12 element-typed
/// composite roles. Built once: the set asks it for every application.
fn containment_table() -> &'static BTreeSet<(ElementKind, ElementKind)> {
    static TABLE: OnceLock<BTreeSet<(ElementKind, ElementKind)>> = OnceLock::new();
    TABLE.get_or_init(build_containment_table)
}

fn build_containment_table() -> BTreeSet<(ElementKind, ElementKind)> {
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
// The reference ends, written from the standard
// ---------------------------------------------------------------------------

/// One reference end: the kinds that own it, the kinds its declared type
/// admits, and its bounds (`upper` `None` for `*`).
struct EndRule {
    end: ReferenceEnd,
    owners: &'static [ElementKind],
    declared: &'static [ElementKind],
    lower: usize,
    upper: Option<usize>,
}

/// The ten ends (ptc/22-03-13; formal/23-05-08 §8.2, §11.2, §11.9, §11.10,
/// §11.12, §11.13, §12.3).
static END_RULES: [EndRule; 10] = [
    // §8.2 `citedElement:SACMElement[0..1]`.
    EndRule {
        end: ReferenceEnd::CitedElement,
        owners: &ALL_KINDS,
        declared: &ALL_KINDS,
        lower: 0,
        upper: Some(1),
    },
    // §8.2 `abstractForm:SACMElement[0..1]`.
    EndRule {
        end: ReferenceEnd::AbstractForm,
        owners: &ALL_KINDS,
        declared: &ALL_KINDS,
        lower: 0,
        upper: Some(1),
    },
    // §11.10 `metaClaim:Claim[0..*]`.
    EndRule {
        end: ReferenceEnd::MetaClaim,
        owners: &ASSERTIONS,
        declared: &[Claim],
        lower: 0,
        upper: None,
    },
    // §11.13 `reasoning:ArgumentReasoning[0..1]`.
    EndRule {
        end: ReferenceEnd::Reasoning,
        owners: &RELATIONSHIPS,
        declared: &[ArgumentReasoning],
        lower: 0,
        upper: Some(1),
    },
    // §11.13 `source:ArgumentAsset[1..*]`.
    EndRule {
        end: ReferenceEnd::Source,
        owners: &RELATIONSHIPS,
        declared: &ARGUMENT_ASSETS,
        lower: 1,
        upper: None,
    },
    // §11.13 `target:ArgumentAsset[1]`.
    EndRule {
        end: ReferenceEnd::Target,
        owners: &RELATIONSHIPS,
        declared: &ARGUMENT_ASSETS,
        lower: 1,
        upper: Some(1),
    },
    // §11.12 `structure:ArgumentPackage[0..1]`.
    EndRule {
        end: ReferenceEnd::Structure,
        owners: &[ArgumentReasoning],
        declared: &[ArgumentPackage],
        lower: 0,
        upper: Some(1),
    },
    // §11.9 `referencedArtifactElement:Base::ArtifactElement[0..*]`: every
    // ModelElement (ADR 0014).
    EndRule {
        end: ReferenceEnd::ReferencedArtifactElement,
        owners: &[ArtifactReference],
        declared: &MODEL_ELEMENTS,
        lower: 0,
        upper: None,
    },
    // §11.2 `argumentElement:ArgumentationElement[0..*]` of an ArgumentGroup.
    EndRule {
        end: ReferenceEnd::GroupMember,
        owners: &[ArgumentGroup],
        declared: &ARGUMENTATION_ELEMENTS,
        lower: 0,
        upper: None,
    },
    // §12.3 `artifactElement:ArtifactElement[0..*]` of an ArtifactGroup: every
    // ModelElement (ADR 0014, ADR 0034).
    EndRule {
        end: ReferenceEnd::ArtifactGroupMember,
        owners: &[ArtifactGroup],
        declared: &MODEL_ELEMENTS,
        lower: 0,
        upper: None,
    },
];

/// The ends slice 0066's documents hold beyond [`END_RULES`], one rule per
/// (end, declaring family), in the order of `ReferenceEnd`: an
/// `ArtifactAssetRelationship`'s `source` and `target` (§12.14
/// `[1..*]`, slice 0057; its artifact fixture holds one), and a package's
/// `interface` (ptc/22-03-13 `[0..*]`), an interface's `implements` (§11.6,
/// §12.5 `[1]`) and a binding's `participantPackage` (§11.5, §12.4
/// `[2..*]`), each typed to the holder's family (ADR 0039 D1).
static SLICE_0066_RULES: [EndRule; 8] = [
    EndRule {
        end: ReferenceEnd::ArtifactSource,
        owners: &[ArtifactAssetRelationship],
        declared: &PROPERTY_HOLDERS,
        lower: 1,
        upper: None,
    },
    EndRule {
        end: ReferenceEnd::ArtifactTarget,
        owners: &[ArtifactAssetRelationship],
        declared: &PROPERTY_HOLDERS,
        lower: 1,
        upper: None,
    },
    EndRule {
        end: ReferenceEnd::Interface,
        owners: &ARGUMENT_PACKAGES,
        declared: &[ArgumentPackageInterface],
        lower: 0,
        upper: None,
    },
    EndRule {
        end: ReferenceEnd::Interface,
        owners: &ARTIFACT_PACKAGES,
        declared: &[ArtifactPackageInterface],
        lower: 0,
        upper: None,
    },
    EndRule {
        end: ReferenceEnd::Implements,
        owners: &[ArgumentPackageInterface],
        declared: &ARGUMENT_PACKAGES,
        lower: 1,
        upper: Some(1),
    },
    EndRule {
        end: ReferenceEnd::Implements,
        owners: &[ArtifactPackageInterface],
        declared: &ARTIFACT_PACKAGES,
        lower: 1,
        upper: Some(1),
    },
    EndRule {
        end: ReferenceEnd::ParticipantPackage,
        owners: &[ArgumentPackageBinding],
        declared: &ARGUMENT_PACKAGES,
        lower: 2,
        upper: None,
    },
    EndRule {
        end: ReferenceEnd::ParticipantPackage,
        owners: &[ArtifactPackageBinding],
        declared: &ARTIFACT_PACKAGES,
        lower: 2,
        upper: None,
    },
];

/// Every rule of the set, in the order of `ReferenceEnd`.
fn end_rules() -> impl Iterator<Item = &'static EndRule> {
    END_RULES.iter().chain(SLICE_0066_RULES.iter())
}

/// The distinct ends of [`end_rules`], in order.
fn ends_in_order() -> Vec<ReferenceEnd> {
    let mut ends: Vec<ReferenceEnd> = Vec::new();
    for rule in end_rules() {
        if !ends.contains(&rule.end) {
            ends.push(rule.end);
        }
    }
    ends
}

/// The rule of `end` on an element of `kind`, if the kind has the end.
fn rule_for(end: ReferenceEnd, kind: ElementKind) -> Option<&'static EndRule> {
    assert!(
        end_rules().any(|rule| rule.end == end),
        "{EDIT_009}: an end this test does not know: {end:?}"
    );
    end_rules().find(|rule| rule.end == end && rule.owners.contains(&kind))
}

/// The rule of `end` on `kind`, which the test expects the kind to own.
fn owned_rule(end: ReferenceEnd, kind: ElementKind) -> &'static EndRule {
    match rule_for(end, kind) {
        Some(rule) => rule,
        None => panic!("{EDIT_009}: setup: a {kind} has no {}", end.name()),
    }
}

/// The ids `element` holds in `rule`'s end: none when its kind does not own
/// the end by `rule`.
fn held_by(element: SacmElement<'_>, rule: &EndRule) -> Vec<ElementId> {
    if rule.owners.contains(&element.kind()) {
        held(element, rule.end)
    } else {
        Vec::new()
    }
}

/// The number of distinct ids of `ids`: a lower bound counts them (ADR
/// 0039 D3).
fn distinct(ids: &[ElementId]) -> usize {
    ids.iter().collect::<BTreeSet<_>>().len()
}

/// The ids `element` holds in `end`, in order, read through the public
/// accessors.
fn held(element: SacmElement<'_>, end: ReferenceEnd) -> Vec<ElementId> {
    fn one(id: Option<&ElementId>) -> Vec<ElementId> {
        id.into_iter().cloned().collect()
    }
    match end {
        ReferenceEnd::CitedElement => one(element.cited_element()),
        ReferenceEnd::AbstractForm => one(element.abstract_form()),
        ReferenceEnd::MetaClaim => element.meta_claims().to_vec(),
        ReferenceEnd::Reasoning => one(element.reasoning()),
        // An ArtifactAssetRelationship's sources are its ArtifactSource
        // (slice 0066's artifact fixture holds one).
        ReferenceEnd::Source if element.kind() == ArtifactAssetRelationship => Vec::new(),
        ReferenceEnd::Source => element.sources().to_vec(),
        ReferenceEnd::Target => one(element.target()),
        ReferenceEnd::Structure => one(element.structure()),
        ReferenceEnd::ReferencedArtifactElement => element.referenced_artifact_elements().to_vec(),
        // `group_members()` answers for both group kinds (ADR 0034): each end
        // is the members of its own group kind only.
        ReferenceEnd::GroupMember if element.kind() == ArgumentGroup => {
            element.group_members().to_vec()
        }
        ReferenceEnd::ArtifactGroupMember if element.kind() == ArtifactGroup => {
            element.group_members().to_vec()
        }
        // The ends of slice 0066's documents: `sources()` and `targets()`
        // answer for both relationship kinds (ADR 0034 amendment 2), so each
        // end is its own kind's only.
        ReferenceEnd::ArtifactSource if element.kind() == ArtifactAssetRelationship => {
            element.sources().to_vec()
        }
        ReferenceEnd::ArtifactTarget if element.kind() == ArtifactAssetRelationship => {
            element.targets().to_vec()
        }
        ReferenceEnd::GroupMember
        | ReferenceEnd::ArtifactGroupMember
        | ReferenceEnd::ArtifactSource
        | ReferenceEnd::ArtifactTarget => Vec::new(),
        ReferenceEnd::Interface => element.interfaces().to_vec(),
        ReferenceEnd::Implements => one(element.implements()),
        ReferenceEnd::ParticipantPackage => element.participant_packages().to_vec(),
        other => panic!("{EDIT_009}: an end this test does not know: {other:?}"),
    }
}

/// Whether `value` holds XML white space (space, TAB, LF, CR).
fn holds_white_space(value: &str) -> bool {
    value.contains([' ', '\t', '\n', '\r'])
}

/// The three flags.
const FLAGS: [Flag; 3] = [Flag::IsCitation, Flag::IsAbstract, Flag::IsCounter];

// ---------------------------------------------------------------------------
// Structural validity (LIB-EDIT-002), through the public views only
// ---------------------------------------------------------------------------

/// Asserts the structural checks S1-S4 and S7 on `doc`. `row` and `context`
/// start every failure message.
fn assert_structurally_valid(row: &str, context: &str, doc: &Document) {
    let elements: Vec<SacmElement<'_>> = doc.elements().collect();
    assert_ids_are_unique_and_found(row, context, doc, &elements);
    assert_links_agree(row, context, doc, &elements);
    assert_kinds_are_legal(row, context, doc, &elements);
    assert_assertions_have_declarations(row, context, &elements);
    assert_xmi_ids_are_unique(row, context, &elements);
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

/// Every `xmi:id` an element carries: its own, then those of its name, its
/// `content` and its values, and its `key` and its values.
fn xmi_ids_of(element: SacmElement<'_>) -> Vec<&ElementId> {
    let mut ids = vec![element.id()];
    ids.extend(element.name_id());
    ids.extend(element.content_id());
    ids.extend(element.content_value_ids().iter().flatten());
    ids.extend(element.key_id());
    ids.extend(element.key_value_ids().iter().flatten());
    // A Resource's `location` and its values (SACM23-ART-005).
    ids.extend(element.location_id());
    ids.extend(element.location_value_ids().iter().flatten());
    ids
}

/// S7: every `xmi:id` in the document is unique.
fn assert_xmi_ids_are_unique(row: &str, context: &str, elements: &[SacmElement<'_>]) {
    let mut seen = BTreeSet::new();
    for &element in elements {
        for id in xmi_ids_of(element) {
            assert!(
                seen.insert(id.as_str()),
                "{row}: {context}: S7: the xmi:id {id:?} is used more than once (last by {:?})",
                element.id()
            );
        }
    }
}

/// Asserts S5, S6, S8 and S9 of a successful `operation` from `before` to
/// `after`.
fn assert_edit_kept_structure(
    row: &str,
    context: &str,
    operation: &Operation,
    before: &Document,
    after: &Document,
) {
    assert_only_a_delete_removes_elements(row, context, operation, before, after);
    assert_names_kept(row, context, before, after);
    assert_keys_kept(row, context, before, after);
    assert_no_new_bad_references(row, context, before, after);
    assert_required_ends_kept(row, context, before, after);
}

/// S9: an operation other than a `DeleteElement` keeps every element.
fn assert_only_a_delete_removes_elements(
    row: &str,
    context: &str,
    operation: &Operation,
    before: &Document,
    after: &Document,
) {
    if matches!(operation, Operation::DeleteElement(_)) {
        return;
    }
    for element in before.elements() {
        assert!(
            after.contains(element.id()),
            "{row}: {context}: S9: the {} {:?} is gone, and the operation is no delete",
            element.kind(),
            element.id()
        );
    }
}

/// S5, names: every surviving `ModelElement` that had a name still has one
/// (S9 checks that only a delete removes elements).
fn assert_names_kept(row: &str, context: &str, before: &Document, after: &Document) {
    for element in before.elements() {
        let Some(now) = after.get(element.id()) else {
            continue;
        };
        if element.name().is_some() {
            let kept = now.name();
            assert!(
                kept.is_some(),
                "{row}: {context}: S5: the {} {:?} lost its name",
                element.kind(),
                element.id()
            );
        }
    }
}

/// S5, keys: every surviving `TaggedValue` that had a key still has one.
fn assert_keys_kept(row: &str, context: &str, before: &Document, after: &Document) {
    for element in before.elements() {
        let Some(now) = after.get(element.id()) else {
            continue;
        };
        if element.kind() == TaggedValue && element.key().is_some() {
            let kept = now.key();
            assert!(
                kept.is_some(),
                "{row}: {context}: S5: the TaggedValue {:?} lost its key",
                element.id()
            );
        }
    }
}

/// Every (owner, end, id) of `doc` whose id names no element or an element
/// its end's declared type does not admit, by [`END_RULES`].
fn bad_references(doc: &Document) -> BTreeSet<(ElementId, &'static str, ElementId)> {
    let mut bad = BTreeSet::new();
    for element in doc.elements() {
        for rule in end_rules() {
            for target in held_by(element, rule) {
                let admitted = doc
                    .get(&target)
                    .is_some_and(|found| rule.declared.contains(&found.kind()));
                if !admitted {
                    bad.insert((element.id().clone(), rule.end.name(), target));
                }
            }
        }
    }
    bad
}

/// S6: no dangling or wrongly typed reference is new (D-VAL-LAYER).
fn assert_no_new_bad_references(row: &str, context: &str, before: &Document, after: &Document) {
    let was = bad_references(before);
    let new: Vec<_> = bad_references(after).difference(&was).cloned().collect();
    assert_eq!(
        new,
        [],
        "{row}: {context}: S6: the operation made these (owner, end, id) dangling or wrongly typed"
    );
}

/// S8: no surviving asserted relationship loses its last source or its
/// target (S9 checks that only a delete removes elements).
fn assert_required_ends_kept(row: &str, context: &str, before: &Document, after: &Document) {
    for element in before.elements() {
        if !RELATIONSHIPS.contains(&element.kind()) {
            continue;
        }
        let Some(now) = after.get(element.id()) else {
            continue;
        };
        assert!(
            element.sources().is_empty() || !now.sources().is_empty(),
            "{row}: {context}: S8: the relationship {:?} lost its last source",
            element.id()
        );
        assert!(
            element.target().is_none() || now.target().is_some(),
            "{row}: {context}: S8: the relationship {:?} lost its target",
            element.id()
        );
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

/// The content the content edits set: two values in two languages.
fn two_values() -> MultiLangString {
    [
        LangString::new("Reviewed").with_lang("en"),
        LangString::new("Granskad").with_lang("sv"),
    ]
    .into_iter()
    .collect()
}

/// The key the key edits set.
fn a_key() -> MultiLangString {
    MultiLangString::from(LangString::new("SIL").with_lang("en"))
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

/// Applies an edit the test expects to succeed during setup.
fn setup_edit(row: &str, doc: &mut Document, operation: impl Into<Operation>) {
    let operation = operation.into();
    if let Err(error) = edit::apply(doc, operation.clone()) {
        panic!("{row}: setup: {operation:?} was refused: {error} ({error:?})");
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

/// The ids the next generated element of each of the 20 kinds the edit API
/// creates would get, observed on a clone so that `doc` is untouched: those
/// of [`next_generated_ids_of_every_kind`].
fn next_generated_ids(row: &str, doc: &Document) -> Vec<ElementId> {
    next_generated_ids_of_every_kind(row, doc)
}

/// The position of `kind` in [`next_generated_ids`]: the order of
/// [`creates_of_every_kind`].
fn probe_index(kind: ElementKind) -> usize {
    const ORDER: [ElementKind; CREATE_COUNT] = [
        AssuranceCasePackage,
        ArgumentPackage,
        Claim,
        ArgumentReasoning,
        ArtifactReference,
        ArgumentGroup,
        AssertedInference,
        AssertedContext,
        AssertedEvidence,
        AssertedArtifactSupport,
        AssertedArtifactContext,
        Description,
        ImplementationConstraint,
        Note,
        TaggedValue,
        ArtifactPackage,
        Property,
        Technique,
        Participant,
        ArtifactGroup,
        Artifact,
        Event,
        Resource,
        Activity,
        // The creates of slice 0066, probed last.
        ArgumentPackageInterface,
        ArgumentPackageBinding,
        ArtifactPackageInterface,
        ArtifactPackageBinding,
    ];
    match ORDER.iter().position(|&created| created == kind) {
        Some(index) => index,
        None => panic!("{EDIT_003}: the edit API does not create a {kind}"),
    }
}

/// What an operation of the set is: a create, with the kind it creates, its
/// parent (`None` for a root) and its caller id (`None` for a generated
/// one); or an edit of an existing element, with its coverage label.
enum Class<'a> {
    Create {
        kind: ElementKind,
        parent: Option<&'a ElementId>,
        id: Option<&'a ElementId>,
    },
    Edit {
        label: &'static str,
    },
}

/// Classifies `operation`.
#[expect(clippy::too_many_lines, reason = "one arm per operation")]
fn classify(operation: &Operation) -> Class<'_> {
    let edit_of = |label| Class::Edit { label };
    // A create with a parent.
    macro_rules! under {
        ($create:expr, $kind:expr) => {
            Class::Create {
                kind: $kind,
                parent: Some($create.parent()),
                id: $create.id(),
            }
        };
    }
    match operation {
        Operation::CreateArgumentReasoning(create) => under!(create, ArgumentReasoning),
        Operation::CreateArtifactReference(create) => under!(create, ArtifactReference),
        Operation::CreateArgumentGroup(create) => under!(create, ArgumentGroup),
        Operation::CreateAssertedInference(create) => under!(create, AssertedInference),
        Operation::CreateAssertedContext(create) => under!(create, AssertedContext),
        Operation::CreateAssertedEvidence(create) => under!(create, AssertedEvidence),
        Operation::CreateAssertedArtifactSupport(create) => {
            under!(create, AssertedArtifactSupport)
        }
        Operation::CreateAssertedArtifactContext(create) => {
            under!(create, AssertedArtifactContext)
        }
        Operation::CreateDescription(create) => under!(create, Description),
        Operation::CreateImplementationConstraint(create) => {
            under!(create, ImplementationConstraint)
        }
        Operation::CreateNote(create) => under!(create, Note),
        Operation::CreateTaggedValue(create) => under!(create, TaggedValue),
        Operation::CreateProperty(create) => under!(create, Property),
        Operation::CreateTechnique(create) => under!(create, Technique),
        Operation::CreateParticipant(create) => under!(create, Participant),
        Operation::CreateArtifactGroup(create) => under!(create, ArtifactGroup),
        Operation::CreateArtifact(create) => under!(create, Artifact),
        Operation::CreateEvent(create) => under!(create, Event),
        Operation::CreateResource(create) => under!(create, Resource),
        Operation::CreateActivity(create) => under!(create, Activity),
        Operation::CreateArtifactPackage(create) => Class::Create {
            kind: ArtifactPackage,
            parent: create.parent(),
            id: create.id(),
        },
        Operation::CreateAssuranceCasePackage(create) => Class::Create {
            kind: AssuranceCasePackage,
            parent: create.parent(),
            id: create.id(),
        },
        Operation::CreateArgumentPackage(create) => Class::Create {
            kind: ArgumentPackage,
            parent: create.parent(),
            id: create.id(),
        },
        Operation::CreateClaim(create) => Class::Create {
            kind: Claim,
            parent: Some(create.parent()),
            id: create.id(),
        },
        // The creates of slice 0066, each a root or under a parent.
        Operation::CreateArgumentPackageInterface(create) => Class::Create {
            kind: ArgumentPackageInterface,
            parent: create.parent(),
            id: create.id(),
        },
        Operation::CreateArgumentPackageBinding(create) => Class::Create {
            kind: ArgumentPackageBinding,
            parent: create.parent(),
            id: create.id(),
        },
        Operation::CreateArtifactPackageInterface(create) => Class::Create {
            kind: ArtifactPackageInterface,
            parent: create.parent(),
            id: create.id(),
        },
        Operation::CreateArtifactPackageBinding(create) => Class::Create {
            kind: ArtifactPackageBinding,
            parent: create.parent(),
            id: create.id(),
        },
        Operation::SetName(_) => edit_of("SetName"),
        Operation::SetGid(set) if set.gid().is_some() => edit_of("SetGid"),
        Operation::SetGid(_) => edit_of("SetGid::remove"),
        Operation::SetFlag(_) => edit_of("SetFlag"),
        Operation::SetAssertionDeclaration(_) => edit_of("SetAssertionDeclaration"),
        Operation::SetContent(set) if set.content().is_some() => edit_of("SetContent"),
        Operation::SetContent(_) => edit_of("SetContent::remove"),
        Operation::SetKey(_) => edit_of("SetKey"),
        Operation::SetVersion(set) if set.version().is_some() => edit_of("SetVersion"),
        Operation::SetVersion(_) => edit_of("SetVersion::remove"),
        Operation::SetDate(set) if set.date().is_some() => edit_of("SetDate"),
        Operation::SetDate(_) => edit_of("SetDate::remove"),
        Operation::SetLocation(set) if set.location().is_some() => edit_of("SetLocation"),
        Operation::SetLocation(_) => edit_of("SetLocation::remove"),
        Operation::SetStartTime(set) if set.start_time().is_some() => edit_of("SetStartTime"),
        Operation::SetStartTime(_) => edit_of("SetStartTime::remove"),
        Operation::SetEndTime(set) if set.end_time().is_some() => edit_of("SetEndTime"),
        Operation::SetEndTime(_) => edit_of("SetEndTime::remove"),
        Operation::SetReferences(_) => edit_of("SetReferences"),
        Operation::AddReference(_) => edit_of("AddReference"),
        Operation::RemoveReference(_) => edit_of("RemoveReference"),
        Operation::ReorderElements(reorder) if reorder.parent().is_some() => {
            edit_of("ReorderElements")
        }
        Operation::ReorderElements(_) => edit_of("ReorderElements::roots"),
        Operation::MoveElement(moved) if moved.parent().is_some() => edit_of("MoveElement"),
        Operation::MoveElement(_) => edit_of("MoveElement::root"),
        Operation::CopyElements(copy) if copy.parent().is_some() => edit_of("CopyElements"),
        Operation::CopyElements(_) => edit_of("CopyElements::root"),
        Operation::DeleteElement(_) => edit_of("DeleteElement"),
        other => panic!("{EDIT_002}: an operation this test does not know: {other:?}"),
    }
}

/// The coverage labels of the attribute edits (LIB-EDIT-008).
const ATTRIBUTE_EDITS: [&str; 5] = [
    "SetName",
    "SetGid",
    "SetGid::remove",
    "SetFlag",
    "SetAssertionDeclaration",
];

/// The coverage labels of the reference edits (LIB-EDIT-009).
const REFERENCE_EDITS: [&str; 3] = ["SetReferences", "AddReference", "RemoveReference"];

/// The coverage labels of the reorders (LIB-EDIT-010).
const REORDERS: [&str; 2] = ["ReorderElements", "ReorderElements::roots"];

/// The coverage labels of the content edits (LIB-EDIT-012).
const CONTENT_EDITS: [&str; 3] = ["SetContent", "SetContent::remove", "SetKey"];

/// The coverage label of the deletes (LIB-EDIT-005, LIB-EDIT-006).
const DELETES: [&str; 1] = ["DeleteElement"];

/// The coverage labels of the moves (LIB-EDIT-017): under a parent, and to
/// the roots.
const MOVES: [&str; 2] = ["MoveElement", "MoveElement::root"];

/// The coverage labels of the copies (LIB-EDIT-018): under a parent, and as
/// roots.
const COPIES: [&str; 2] = ["CopyElements", "CopyElements::root"];

/// The created kinds that are no package and no `UtilityElement` and whose
/// create gives no reference end: a Claim, and the `ArtifactAsset`s, which
/// have no end of their own (§12.6).
const CREATES_WITHOUT_ENDS: &[ElementKind] = &[
    Claim,
    Property,
    Technique,
    Participant,
    Artifact,
    Event,
    Resource,
    Activity,
];

/// The coverage labels of the value edits of slice 0055 (SACM23-ART-002,
/// -004, -005; ADR 0034 amendment 1), which join the LIB-EDIT-002 set.
const ASSET_VALUE_EDITS: [&str; 10] = [
    "SetVersion",
    "SetVersion::remove",
    "SetDate",
    "SetDate::remove",
    "SetLocation",
    "SetLocation::remove",
    "SetStartTime",
    "SetStartTime::remove",
    "SetEndTime",
    "SetEndTime::remove",
];

/// The answers the set must reach for the edit labelled `label`: `Ok` and
/// CMD-001 always; CMD-008 for an operation whose feature some kind lacks;
/// MULT-001 for the reference edits; ID-002 and REF-002 for those that check
/// the ids given; CMD-009 for the reorders; CMD-002, CMD-006 and CMD-007 for
/// the deletes.
fn required_answers(label: &str) -> Vec<Result<(), &'static str>> {
    let mut required = vec![Ok(()), Err(Code::CMD_TARGET_NOT_FOUND.as_str())];
    // A move: the element and the parent are known (CMD-001), the parent
    // can hold it or a root is a package, and it is not moved into itself
    // (CMD-005), and the position is in range (CMD-009).
    if MOVES.contains(&label) {
        required.push(Err(Code::CMD_INVALID_PARENT.as_str()));
        required.push(Err(Code::CMD_INVALID_ORDER.as_str()));
        return required;
    }
    // A copy under a parent: the parent is known and every reference names
    // an element (CMD-001), the parent can hold each top (CMD-005), every
    // relationship has its required ends (MULT-001), and every reference
    // names an element of a kind its end admits (REF-002). As roots: each
    // top is a package (CMD-005). No preserved content is in the set, and
    // no end can hold an id with white space, so CMD-004 and ID-002 are
    // reached in `copy_elements.rs` only.
    if label == "CopyElements" {
        required.push(Err(Code::CMD_INVALID_PARENT.as_str()));
        required.push(Err(Code::MULTIPLICITY_VIOLATION.as_str()));
        required.push(Err(Code::REF_WRONG_TYPE.as_str()));
        return required;
    }
    if label == "CopyElements::root" {
        return vec![Ok(()), Err(Code::CMD_INVALID_PARENT.as_str())];
    }
    if DELETES.contains(&label) {
        required.push(Err(Code::CMD_DELETE_REFERENCED.as_str()));
        required.push(Err(Code::CMD_PACKAGE_NOT_EMPTY.as_str()));
        required.push(Err(Code::CMD_EXTERNAL_REFERENCES.as_str()));
        return required;
    }
    if !matches!(
        label,
        "SetGid" | "SetGid::remove" | "ReorderElements" | "ReorderElements::roots"
    ) {
        required.push(Err(Code::CMD_FEATURE_NOT_APPLICABLE.as_str()));
    }
    if REFERENCE_EDITS.contains(&label) {
        required.push(Err(Code::MULTIPLICITY_VIOLATION.as_str()));
    }
    if matches!(label, "SetReferences" | "AddReference") {
        required.push(Err(Code::ID_INVALID.as_str()));
        required.push(Err(Code::REF_WRONG_TYPE.as_str()));
    }
    if REORDERS.contains(&label) {
        required.push(Err(Code::CMD_INVALID_ORDER.as_str()));
    }
    required
}

/// Whether the set must reach an `Ok` of `label` with empty `changed_ids`
/// (an add and a remove always change their end).
fn requires_unchanged_ok(label: &str) -> bool {
    !matches!(label, "AddReference" | "RemoveReference")
}

/// Whether the set must reach an `Ok` of `label` with `changed_ids` not
/// empty (a root reorder reports no id, and a copy changes nothing).
fn requires_changed_ok(label: &str) -> bool {
    label != "ReorderElements::roots" && !COPIES.contains(&label)
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

// ---------------------------------------------------------------------------
// The oracle: what each operation must answer
// ---------------------------------------------------------------------------

/// What `edit::apply` must answer for `operation` on `doc`, by this file's
/// own tables and the check orders of ADR 0019 and ADR 0029.
fn expected_answer(doc: &Document, operation: &Operation) -> Result<(), Code> {
    match operation {
        Operation::SetName(set) => {
            feature_answer(doc, set.element(), |kind| MODEL_ELEMENTS.contains(&kind))
        }
        Operation::SetGid(set) => feature_answer(doc, set.element(), |_| true),
        Operation::SetFlag(set) => {
            let counter = set.flag() == Flag::IsCounter;
            feature_answer(doc, set.element(), |kind| {
                !counter || RELATIONSHIPS.contains(&kind)
            })
        }
        Operation::SetAssertionDeclaration(set) => {
            feature_answer(doc, set.element(), |kind| ASSERTIONS.contains(&kind))
        }
        Operation::SetContent(set) => {
            feature_answer(doc, set.element(), |kind| UTILITY_ELEMENTS.contains(&kind))
        }
        Operation::SetKey(set) => feature_answer(doc, set.element(), |kind| kind == TaggedValue),
        // §12.7 `version`, §12.7 and §12.9 `date`, §12.10 `location`, §12.11
        // `startTime` and `endTime`.
        Operation::SetVersion(set) => feature_answer(doc, set.element(), |kind| kind == Artifact),
        Operation::SetDate(set) => {
            feature_answer(doc, set.element(), |kind| kind == Artifact || kind == Event)
        }
        Operation::SetLocation(set) => feature_answer(doc, set.element(), |kind| kind == Resource),
        Operation::SetStartTime(set) => feature_answer(doc, set.element(), |kind| kind == Activity),
        Operation::SetEndTime(set) => feature_answer(doc, set.element(), |kind| kind == Activity),
        Operation::SetReferences(set) => {
            set_references_answer(doc, set.element(), set.end(), set.ids())
        }
        Operation::AddReference(add) => {
            add_reference_answer(doc, add.element(), add.end(), add.id())
        }
        Operation::RemoveReference(remove) => {
            remove_reference_answer(doc, remove.element(), remove.end(), remove.id())
        }
        Operation::ReorderElements(reorder) => {
            reorder_answer(doc, reorder.parent(), reorder.order())
        }
        Operation::MoveElement(moved) => move_answer(doc, moved),
        Operation::CopyElements(copy) => copy_answer(doc, copy),
        Operation::DeleteElement(delete) => delete_plan(doc, delete).map(drop),
        _ => create_answer(doc, operation),
    }
}

/// A create: ADR 0019's order.
fn create_answer(doc: &Document, operation: &Operation) -> Result<(), Code> {
    let Class::Create {
        kind,
        parent,
        id: caller_id,
    } = classify(operation)
    else {
        panic!("{EDIT_002}: {operation:?} is not a create");
    };
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
    // ADR 0027's checks of the ends, then ADR 0028's typing: a relationship
    // has a source; no id holds white space; every id names an element; and,
    // only then, every id names an element its end's declared type admits.
    let ends = create_ends(operation);
    if RELATIONSHIPS.contains(&kind)
        && ends
            .iter()
            .all(|(end, ids)| *end != ReferenceEnd::Source || ids.is_empty())
    {
        return Err(Code::MULTIPLICITY_VIOLATION);
    }
    // A binding has two distinct participants (ADR 0039 D3); an interface's
    // one `implements` is always given.
    for (end, ids) in &ends {
        let rule = owned_rule(*end, kind);
        if ids.iter().collect::<BTreeSet<_>>().len() < rule.lower {
            return Err(Code::MULTIPLICITY_VIOLATION);
        }
    }
    let all_ids = || ends.iter().flat_map(|(_, ids)| ids.iter().copied());
    if all_ids().any(|value| holds_white_space(value.as_str())) {
        return Err(Code::ID_INVALID);
    }
    if all_ids().any(|value| !doc.contains(value)) {
        return Err(Code::CMD_TARGET_NOT_FOUND);
    }
    for (end, ids) in &ends {
        let declared = owned_rule(*end, kind).declared;
        if ids.iter().any(|value| {
            doc.get(value)
                .is_some_and(|found| !declared.contains(&found.kind()))
        }) {
            return Err(Code::REF_WRONG_TYPE);
        }
    }
    Ok(())
}

/// The reference ends a create gives, each with its ids in order.
fn create_ends(operation: &Operation) -> Vec<(ReferenceEnd, Vec<&ElementId>)> {
    macro_rules! relationship {
        ($create:expr) => {
            vec![
                (
                    ReferenceEnd::Reasoning,
                    $create.reasoning().into_iter().collect(),
                ),
                (ReferenceEnd::Source, $create.sources().iter().collect()),
                (ReferenceEnd::Target, vec![$create.target()]),
            ]
        };
    }
    match operation {
        Operation::CreateArgumentReasoning(create) => vec![(
            ReferenceEnd::Structure,
            create.structure().into_iter().collect(),
        )],
        Operation::CreateArtifactReference(create) => vec![(
            ReferenceEnd::ReferencedArtifactElement,
            create.referenced_artifact_elements().iter().collect(),
        )],
        Operation::CreateArgumentGroup(create) => vec![(
            ReferenceEnd::GroupMember,
            create.group_members().iter().collect(),
        )],
        Operation::CreateArtifactGroup(create) => vec![(
            ReferenceEnd::ArtifactGroupMember,
            create.group_members().iter().collect(),
        )],
        Operation::CreateAssertedInference(create) => relationship!(create),
        Operation::CreateAssertedContext(create) => relationship!(create),
        Operation::CreateAssertedEvidence(create) => relationship!(create),
        Operation::CreateAssertedArtifactSupport(create) => relationship!(create),
        Operation::CreateAssertedArtifactContext(create) => relationship!(create),
        // The creates of slice 0066 (ADR 0039 D4).
        Operation::CreateArgumentPackageInterface(create) => {
            vec![(ReferenceEnd::Implements, vec![create.implements()])]
        }
        Operation::CreateArtifactPackageInterface(create) => {
            vec![(ReferenceEnd::Implements, vec![create.implements()])]
        }
        Operation::CreateArgumentPackageBinding(create) => vec![(
            ReferenceEnd::ParticipantPackage,
            create.participant_packages().iter().collect(),
        )],
        Operation::CreateArtifactPackageBinding(create) => vec![(
            ReferenceEnd::ParticipantPackage,
            create.participant_packages().iter().collect(),
        )],
        _ => Vec::new(),
    }
}

/// An attribute or content edit: the element is known, then its kind has
/// the feature. No value is refused.
fn feature_answer(
    doc: &Document,
    element: &ElementId,
    has_feature: impl Fn(ElementKind) -> bool,
) -> Result<(), Code> {
    let Some(found) = doc.get(element) else {
        return Err(Code::CMD_TARGET_NOT_FOUND);
    };
    if !has_feature(found.kind()) {
        return Err(Code::CMD_FEATURE_NOT_APPLICABLE);
    }
    Ok(())
}

/// The element `element` of `doc` and the rule of `end`, once the element is
/// known and owns the end.
fn owner_of<'d>(
    doc: &'d Document,
    element: &ElementId,
    end: ReferenceEnd,
) -> Result<(SacmElement<'d>, &'static EndRule), Code> {
    let Some(found) = doc.get(element) else {
        // The end must be one this test knows, whatever the element.
        let _ = rule_for(end, Claim);
        return Err(Code::CMD_TARGET_NOT_FOUND);
    };
    match rule_for(end, found.kind()) {
        Some(rule) => Ok((found, rule)),
        None => Err(Code::CMD_FEATURE_NOT_APPLICABLE),
    }
}

/// The checks of the ids an end does not hold yet, as the creates run them:
/// white space in any, then any naming no element, then any naming an
/// element its end does not admit.
fn new_ids_answer(doc: &Document, rule: &EndRule, new: &[&ElementId]) -> Result<(), Code> {
    if new.iter().any(|id| holds_white_space(id.as_str())) {
        return Err(Code::ID_INVALID);
    }
    if new.iter().any(|id| !doc.contains(*id)) {
        return Err(Code::CMD_TARGET_NOT_FOUND);
    }
    let admitted = |id: &&ElementId| {
        doc.get(*id)
            .is_some_and(|found| rule.declared.contains(&found.kind()))
    };
    if !new.iter().all(admitted) {
        return Err(Code::REF_WRONG_TYPE);
    }
    Ok(())
}

fn set_references_answer(
    doc: &Document,
    element: &ElementId,
    end: ReferenceEnd,
    ids: &[ElementId],
) -> Result<(), Code> {
    let (found, rule) = owner_of(doc, element, end)?;
    // A lower bound counts distinct ids, an upper bound every id (ADR 0039
    // D3); the two counts differ only for an end whose lower bound is above 1.
    if distinct(ids) < rule.lower {
        return Err(Code::MULTIPLICITY_VIOLATION);
    }
    if rule.upper.is_some_and(|upper| ids.len() > upper) {
        return Err(Code::MULTIPLICITY_VIOLATION);
    }
    let held = held(found, end);
    let new: Vec<&ElementId> = ids.iter().filter(|id| !held.contains(id)).collect();
    new_ids_answer(doc, rule, &new)
}

fn add_reference_answer(
    doc: &Document,
    element: &ElementId,
    end: ReferenceEnd,
    added: &ElementId,
) -> Result<(), Code> {
    let (found, rule) = owner_of(doc, element, end)?;
    let held = held(found, end);
    if rule.upper.is_some_and(|upper| held.len() + 1 > upper) {
        return Err(Code::MULTIPLICITY_VIOLATION);
    }
    // The end after the add must hold its lower bound of distinct ids: a
    // binding read with one participant is not repaired by adding it again
    // (ADR 0039 Consequences).
    let mut after = held.clone();
    after.push(added.clone());
    if distinct(&after) < rule.lower {
        return Err(Code::MULTIPLICITY_VIOLATION);
    }
    if held.contains(added) {
        return Ok(());
    }
    new_ids_answer(doc, rule, &[added])
}

fn remove_reference_answer(
    doc: &Document,
    element: &ElementId,
    end: ReferenceEnd,
    removed: &ElementId,
) -> Result<(), Code> {
    let (found, rule) = owner_of(doc, element, end)?;
    let held = held(found, end);
    if !held.contains(removed) {
        return Err(Code::CMD_TARGET_NOT_FOUND);
    }
    let remaining: Vec<ElementId> = held.iter().filter(|id| *id != removed).cloned().collect();
    if distinct(&remaining) < rule.lower {
        return Err(Code::MULTIPLICITY_VIOLATION);
    }
    Ok(())
}

/// A reorder: the parent is known (not for the roots), the list is not
/// empty, then id by id: a direct child (or root), not named before.
fn reorder_answer(
    doc: &Document,
    parent: Option<&ElementId>,
    order: &[ElementId],
) -> Result<(), Code> {
    let children: Vec<&ElementId> = match parent {
        Some(parent) => {
            let Some(parent) = doc.get(parent) else {
                return Err(Code::CMD_TARGET_NOT_FOUND);
            };
            parent.children().map(SacmElement::id).collect()
        }
        None => doc.roots().map(SacmElement::id).collect(),
    };
    if order.is_empty() {
        return Err(Code::CMD_INVALID_ORDER);
    }
    let mut seen = BTreeSet::new();
    for child in order {
        if !children.contains(&child) {
            return Err(Code::CMD_TARGET_NOT_FOUND);
        }
        if !seen.insert(child) {
            return Err(Code::CMD_INVALID_ORDER);
        }
    }
    Ok(())
}

// ---------------------------------------------------------------------------
// The move and copy oracles, written from ADR 0036
// ---------------------------------------------------------------------------

/// A move (ADR 0036 D1): the element is known; under a parent, the parent
/// is known and its kind can hold the element's, by this file's containment
/// table, or for the roots the element is a package; the parent is not the
/// element and not inside it; and the position is at most the number of
/// the others.
fn move_answer(doc: &Document, moved: &MoveElement) -> Result<(), Code> {
    let Some(element) = doc.get(moved.element()) else {
        return Err(Code::CMD_TARGET_NOT_FOUND);
    };
    let others = if let Some(parent) = moved.parent() {
        let Some(found) = doc.get(parent) else {
            return Err(Code::CMD_TARGET_NOT_FOUND);
        };
        if !containment_table().contains(&(found.kind(), element.kind())) {
            return Err(Code::CMD_INVALID_PARENT);
        }
        if subtree_of(element).contains(parent) {
            return Err(Code::CMD_INVALID_PARENT);
        }
        found
            .children()
            .filter(|child| child.id() != element.id())
            .count()
    } else {
        if !PACKAGE_KINDS.contains(&element.kind()) {
            return Err(Code::CMD_INVALID_PARENT);
        }
        doc.roots().filter(|root| root.id() != element.id()).count()
    };
    if moved.position().is_some_and(|position| position > others) {
        return Err(Code::CMD_INVALID_ORDER);
    }
    Ok(())
}

/// Where a copy of the set came from: the source document, and whether it
/// is the starting document itself (rather than [`COPY_SOURCE`]).
struct CopySource {
    copy: CopyElements,
    source: Document,
    from_itself: bool,
}

thread_local! {
    /// Every copy [`copy_operations_for`] built on this thread, most recent
    /// last. `CopyElements` keeps its snapshot private, so the oracles look
    /// the source up by the copy.
    static COPY_SOURCES: std::cell::RefCell<Vec<CopySource>> =
        const { std::cell::RefCell::new(Vec::new()) };
}

/// Builds `CopyElements::new` (or `root` for `None`) of `elements` of
/// `source` under `parent`, which must succeed, and remembers its source.
fn remembered_copy(
    source: &Document,
    from_itself: bool,
    elements: &[ElementId],
    parent: Option<&ElementId>,
) -> Operation {
    let built = match parent {
        Some(parent) => CopyElements::new(source, elements.iter().cloned(), parent),
        None => CopyElements::root(source, elements.iter().cloned()),
    };
    let copy = match built {
        Ok(copy) => copy,
        Err(error) => panic!("{EDIT_018}: setup: the copy of {elements:?} was refused: {error}"),
    };
    COPY_SOURCES.with(|sources| {
        sources.borrow_mut().push(CopySource {
            copy: copy.clone(),
            source: source.clone(),
            from_itself,
        });
    });
    copy.into()
}

/// The source of `copy`, which [`remembered_copy`] built on this thread,
/// and whether it was the starting document itself.
fn source_of(copy: &CopyElements) -> (Document, bool) {
    let found = COPY_SOURCES.with(|sources| {
        sources
            .borrow()
            .iter()
            .rev()
            .find(|known| known.copy == *copy)
            .map(|known| (known.source.clone(), known.from_itself))
    });
    match found {
        Some(found) => found,
        None => panic!("{EDIT_018}: setup: a copy this test did not build: {copy:?}"),
    }
}

/// The copied elements of `copy` in `source`, in the order of
/// `source_ids()`.
fn copied_elements<'s>(source: &'s Document, copy: &CopyElements) -> Vec<SacmElement<'s>> {
    copy.source_ids()
        .iter()
        .map(|value| match source.get(value) {
            Some(found) => found,
            None => panic!("{EDIT_018}: source_ids() names {value:?}, which the source lacks"),
        })
        .collect()
}

/// A copy (ADR 0036 D2), checked as a create: the parent is known; each top
/// (a copied element whose parent is not copied), in order, can be held by
/// the parent, or is a package for the roots; (no preserved content is in
/// the set); every copied relationship has a source and a target; then,
/// over every id the copied elements' ends hold, mapped (an id of a copied
/// element names its copy, any other is kept), in three passes: no kept id
/// holds white space, every kept id names an element of the target, and
/// every id names an element of a kind its end admits.
fn copy_answer(doc: &Document, copy: &CopyElements) -> Result<(), Code> {
    let (source, _) = source_of(copy);
    let copied = copied_elements(&source, copy);
    let is_copied = |value: &ElementId| copy.source_ids().contains(value);
    let parent_kind = match copy.parent() {
        Some(parent) => match doc.get(parent) {
            Some(found) => Some(found.kind()),
            None => return Err(Code::CMD_TARGET_NOT_FOUND),
        },
        None => None,
    };
    for top in copied.iter().filter(|element| {
        !element
            .parent()
            .is_some_and(|parent| is_copied(parent.id()))
    }) {
        let legal = match parent_kind {
            Some(kind) => containment_table().contains(&(kind, top.kind())),
            None => PACKAGE_KINDS.contains(&top.kind()),
        };
        if !legal {
            return Err(Code::CMD_INVALID_PARENT);
        }
    }
    assert!(
        !source.has_preserved_content(),
        "{EDIT_018}: setup: a source of the set holds preserved content"
    );
    for element in &copied {
        if RELATIONSHIPS.contains(&element.kind())
            && (element.sources().is_empty() || element.target().is_none())
        {
            return Err(Code::MULTIPLICITY_VIOLATION);
        }
        // Every end holds its lower bound of distinct ids: an interface its
        // implements, a binding two participants (ADR 0039 D3, D6).
        let below_bound = end_rules().any(|rule| {
            rule.owners.contains(&element.kind()) && distinct(&held_by(*element, rule)) < rule.lower
        });
        if below_bound {
            return Err(Code::MULTIPLICITY_VIOLATION);
        }
    }
    // (declared kinds, the id as held, the kind it names after mapping)
    let mut references: Vec<(&[ElementKind], ElementId, Option<ElementKind>)> = Vec::new();
    for element in &copied {
        for rule in end_rules() {
            for value in held_by(*element, rule) {
                let named = if is_copied(&value) {
                    source.get(&value).map(SacmElement::kind)
                } else {
                    doc.get(&value).map(SacmElement::kind)
                };
                references.push((rule.declared, value, named));
            }
        }
    }
    let kept = || references.iter().filter(|(_, value, _)| !is_copied(value));
    if kept().any(|(_, value, _)| holds_white_space(value.as_str())) {
        return Err(Code::ID_INVALID);
    }
    if kept().any(|(_, _, named)| named.is_none()) {
        return Err(Code::CMD_TARGET_NOT_FOUND);
    }
    if references
        .iter()
        .any(|(declared, _, named)| named.is_some_and(|kind| !declared.contains(&kind)))
    {
        return Err(Code::REF_WRONG_TYPE);
    }
    Ok(())
}

/// `kind`'s name in `snake_case`: `AssertedInference` is
/// `asserted_inference`.
fn snake_case(kind: ElementKind) -> String {
    let mut snake = String::new();
    for (index, character) in kind.name().chars().enumerate() {
        if character.is_ascii_uppercase() {
            if index > 0 {
                snake.push('_');
            }
            snake.push(character.to_ascii_lowercase());
        } else {
            snake.push(character);
        }
    }
    snake
}

/// Whether `value` has the form of a generated id of `kind`:
/// `<snake_case kind>_<n>`, `n` at least 1.
fn is_generated_form(kind: ElementKind, value: &str) -> bool {
    value
        .strip_prefix(&format!("{}_", snake_case(kind)))
        .is_some_and(|number| {
            !number.starts_with('0') && number.parse::<u64>().is_ok_and(|n| n >= 1)
        })
}

/// What a successful `copy` from `before` to `after` must have made (ADR
/// 0036 D2): one copy per source id, kept or generated as the id rules
/// say, with every feature but the ids copied exactly, a part's id kept
/// only when free, each reference mapped, and the tops appended to the
/// parent (or the roots) in order.
#[expect(
    clippy::too_many_lines,
    reason = "one check per rule of ADR 0036 D2 reads best as one list"
)]
fn assert_copy_made(
    context: &str,
    before: &Document,
    after: &Document,
    copy: &CopyElements,
    mutation: &Mutation,
) -> (usize, usize) {
    let (source, _) = source_of(copy);
    let copied = copied_elements(&source, copy);
    let sources = copy.source_ids();
    let created = mutation.created_ids();
    assert_eq!(
        created.len(),
        sources.len(),
        "{EDIT_018}: {context}: one created id per source id"
    );
    let mapped = |value: &ElementId| -> ElementId {
        match sources.iter().position(|from| from == value) {
            Some(index) => created[index].clone(),
            None => value.clone(),
        }
    };
    // Free in the target: no element or part holds it, and no reference
    // names it (no preserved content is in the set).
    let in_use: BTreeSet<ElementId> = before
        .elements()
        .flat_map(|element| xmi_ids_of(element).into_iter().cloned().collect::<Vec<_>>())
        .collect();
    let referenced: BTreeSet<ElementId> = before
        .elements()
        .flat_map(|element| {
            end_rules()
                .flat_map(|rule| held_by(element, rule))
                .collect::<Vec<_>>()
        })
        .collect();
    let keeps = |value: &ElementId| {
        is_ascii_ncname(value.as_str()) && !in_use.contains(value) && !referenced.contains(value)
    };
    let named: BTreeSet<ElementId> = copied
        .iter()
        .flat_map(|element| {
            end_rules()
                .flat_map(|rule| held_by(*element, rule))
                .collect::<Vec<_>>()
        })
        .collect();
    let mut kept_ids: BTreeSet<ElementId> = BTreeSet::new();
    for original in &copied {
        kept_ids.extend(
            xmi_ids_of(*original)
                .into_iter()
                .filter(|value| keeps(value))
                .cloned(),
        );
    }
    let (mut kept, mut generated) = (0, 0);
    for (original, to) in copied.iter().zip(created) {
        let from = original.id();
        let Some(made) = after.get(to) else {
            panic!("{EDIT_018}: {context}: the copy {to:?} of {from:?} is not in the document");
        };
        if keeps(from) {
            assert_eq!(
                to, from,
                "{EDIT_018}: {context}: the free id {from:?} is not kept"
            );
            kept += 1;
        } else {
            assert!(
                is_generated_form(original.kind(), to.as_str())
                    && !in_use.contains(to)
                    && !referenced.contains(to)
                    && !kept_ids.contains(to)
                    && !named.contains(to),
                "{EDIT_018}: {context}: the copy of {from:?} has the id {to:?}, which is no \
                 generated id of a {} free in the target and avoiding the ids the copy keeps \
                 and names",
                original.kind()
            );
            generated += 1;
        }
        assert_eq!(
            (
                made.kind(),
                made.name(),
                made.gid(),
                made.is_citation(),
                made.is_abstract(),
                made.is_counter(),
                made.assertion_declaration(),
            ),
            (
                original.kind(),
                original.name(),
                original.gid(),
                original.is_citation(),
                original.is_abstract(),
                original.is_counter(),
                original.assertion_declaration(),
            ),
            "{EDIT_018}: {context}: the features of {to:?}, the copy of {from:?}"
        );
        assert_eq!(
            (made.content(), made.key(), made.location()),
            (original.content(), original.key(), original.location()),
            "{EDIT_018}: {context}: the content, key and location of {to:?}"
        );
        assert_eq!(
            (
                made.version(),
                made.date(),
                made.start_time(),
                made.end_time()
            ),
            (
                original.version(),
                original.date(),
                original.start_time(),
                original.end_time()
            ),
            "{EDIT_018}: {context}: the strings of {to:?}"
        );
        let part = |value: Option<&ElementId>| value.filter(|value| keeps(value)).cloned();
        let parts = |values: &[Option<ElementId>]| -> Vec<Option<ElementId>> {
            values.iter().map(|value| part(value.as_ref())).collect()
        };
        assert_eq!(
            (
                made.name_id().cloned(),
                made.content_id().cloned(),
                made.content_value_ids().to_vec(),
                made.key_id().cloned(),
                made.key_value_ids().to_vec(),
                made.location_id().cloned(),
                made.location_value_ids().to_vec(),
            ),
            (
                part(original.name_id()),
                part(original.content_id()),
                parts(original.content_value_ids()),
                part(original.key_id()),
                parts(original.key_value_ids()),
                part(original.location_id()),
                parts(original.location_value_ids()),
            ),
            "{EDIT_018}: {context}: the part ids of {to:?}: kept when free, dropped otherwise"
        );
        let expected_parent = match original.parent() {
            Some(parent) if sources.contains(parent.id()) => Some(mapped(parent.id())),
            _ => copy.parent().cloned(),
        };
        assert_eq!(
            made.parent().map(|parent| parent.id().clone()),
            expected_parent,
            "{EDIT_018}: {context}: the parent of {to:?}"
        );
        for rule in end_rules() {
            let expected: Vec<ElementId> = held_by(*original, rule).iter().map(mapped).collect();
            assert_eq!(
                held_by(made, rule),
                expected,
                "{EDIT_018}: {context}: the {} of {to:?}: a copied id maps to its copy, any \
                 other is kept",
                rule.end.name()
            );
        }
    }
    // The tops' copies, in order, end the parent's children or the roots.
    let tops: Vec<ElementId> = copied
        .iter()
        .zip(created)
        .filter(|(original, _)| {
            !original
                .parent()
                .is_some_and(|parent| sources.contains(parent.id()))
        })
        .map(|(_, to)| to.clone())
        .collect();
    let siblings: Vec<ElementId> = match copy.parent().and_then(|parent| after.get(parent)) {
        Some(parent) => parent.children().map(|child| child.id().clone()).collect(),
        None => after.roots().map(|root| root.id().clone()).collect(),
    };
    assert!(
        siblings.ends_with(&tops),
        "{EDIT_018}: {context}: the copies {tops:?} are not appended in order: {siblings:?}"
    );
    (kept, generated)
}

// ---------------------------------------------------------------------------
// The delete oracle, written from ADR 0031
// ---------------------------------------------------------------------------

/// The three reference policies, from the strictest.
const REFERENCE_POLICIES: [ReferenceDeletePolicy; 3] = [
    RejectIfReferenced,
    DeleteReferencingRelationships,
    ScrubReferences,
];

/// The two package policies, from the strictest.
const PACKAGE_POLICIES: [PackageDeletePolicy; 2] = [RejectIfNonEmpty, DeleteRecursively];

/// The two cross-package policies, from the strictest.
const CROSS_PACKAGE_POLICIES: [CrossPackageReferencePolicy; 2] =
    [RejectIfExternalReferencesExist, ApplyReferencePolicy];

/// One dangling reference: holder, end, id, and whether it crosses a package
/// boundary.
type Dangling = (ElementId, ReferenceEnd, ElementId, bool);

/// What an accepted delete does, by ADR 0031.
struct DeletePlan {
    /// Every deleted element, in pre-order before.
    deleted: Vec<ElementId>,
    /// Every surviving element that holds a deleted id, in pre-order.
    changed: Vec<ElementId>,
    /// The dangling references, in the order of ADR 0031.
    dangling: Vec<Dangling>,
}

/// `element` and everything it contains, in pre-order.
fn subtree_of(element: SacmElement<'_>) -> Vec<ElementId> {
    let mut ids = vec![element.id().clone()];
    for child in element.children() {
        ids.extend(subtree_of(child));
    }
    ids
}

/// The nearest package of `element`: itself or its closest ancestor of one
/// of the 12 package, interface and binding kinds.
fn nearest_package(element: SacmElement<'_>) -> Option<ElementId> {
    let mut current = Some(element);
    while let Some(found) = current {
        if PACKAGE_KINDS.contains(&found.kind()) {
            return Some(found.id().clone());
        }
        current = found.parent();
    }
    None
}

/// Whether the reference policy `policy` deletes the asserted relationship
/// `relationship` once `deleted` are deleted: under
/// `DeleteReferencingRelationships` when its source, target or reasoning
/// names a deleted element; under `ScrubReferences` when it would be left
/// with no source (it had one) or no target (§11.13).
fn policy_deletes(
    policy: ReferenceDeletePolicy,
    relationship: SacmElement<'_>,
    deleted: &BTreeSet<ElementId>,
) -> bool {
    let sources = relationship.sources();
    let target = relationship.target();
    if policy == DeleteReferencingRelationships {
        sources.iter().any(|id| deleted.contains(id))
            || target.is_some_and(|id| deleted.contains(id))
            || relationship
                .reasoning()
                .is_some_and(|id| deleted.contains(id))
    } else if policy == ScrubReferences {
        (!sources.is_empty() && sources.iter().all(|id| deleted.contains(id)))
            || target.is_some_and(|id| deleted.contains(id))
    } else {
        false
    }
}

/// Whether the reference policy `policy` deletes `holder`, an
/// `ArtifactAssetRelationship`, an interface or a binding, once `deleted` are
/// deleted (ADR 0039 D5): under `DeleteReferencingRelationships` an
/// `ArtifactAssetRelationship` naming a deleted element in `source` or
/// `target`; under either non-reject policy a holder that loses an id from
/// an end with a lower bound and is left below it, counting distinct ids
/// (`source` and `target` [1..*], `implements` [1], `participantPackage`
/// [2..*]). A holder already below its bound that loses no id stays.
fn bound_deletes(
    policy: ReferenceDeletePolicy,
    holder: SacmElement<'_>,
    deleted: &BTreeSet<ElementId>,
) -> bool {
    if policy == RejectIfReferenced {
        return false;
    }
    let relationship_ends = [ReferenceEnd::ArtifactSource, ReferenceEnd::ArtifactTarget];
    end_rules()
        .filter(|rule| rule.lower > 0 && rule.owners.contains(&holder.kind()))
        .filter(|rule| !RELATIONSHIPS.iter().any(|kind| rule.owners.contains(kind)))
        .any(|rule| {
            let ids = held_by(holder, rule);
            let lost = ids.iter().any(|id| deleted.contains(id));
            if policy == DeleteReferencingRelationships && relationship_ends.contains(&rule.end) {
                return lost;
            }
            let remaining: Vec<ElementId> =
                ids.into_iter().filter(|id| !deleted.contains(id)).collect();
            lost && distinct(&remaining) < rule.lower
        })
}

/// What `delete` must answer on `doc`, and what it deletes, changes and
/// leaves dangling when accepted (ADR 0031): `ElementNotFound`; then
/// `NotEmpty` when `RejectIfNonEmpty` meets a contained element that is no
/// `UtilityElement`; then the deleted set, to a fixpoint; then the first
/// dangling reference a policy refuses, in list order.
fn delete_plan(doc: &Document, delete: &DeleteElement) -> Result<DeletePlan, Code> {
    let Some(target) = doc.get(delete.element()) else {
        return Err(Code::CMD_TARGET_NOT_FOUND);
    };
    let subtree: BTreeSet<ElementId> = subtree_of(target).into_iter().collect();
    let holds_a_model_element = subtree
        .iter()
        .filter(|id| *id != target.id())
        .filter_map(|id| doc.get(id))
        .any(|element| !UTILITY_ELEMENTS.contains(&element.kind()));
    if delete.package_policy() == RejectIfNonEmpty && holds_a_model_element {
        return Err(Code::CMD_PACKAGE_NOT_EMPTY);
    }
    let policy = delete.reference_policy();
    let mut deleted = subtree.clone();
    // The interfaces and bindings deleted for their bound (LIB-EDIT-019),
    // which the package policy guards.
    let mut cascade_holders: BTreeSet<ElementId> = BTreeSet::new();
    loop {
        let mut grew = false;
        for element in doc.elements() {
            if deleted.contains(element.id()) {
                continue;
            }
            let relationship = RELATIONSHIPS.contains(&element.kind())
                && policy_deletes(policy, element, &deleted);
            let bound = !relationship && bound_deletes(policy, element, &deleted);
            if !relationship && !bound {
                continue;
            }
            if bound && element.kind() != ArtifactAssetRelationship {
                cascade_holders.insert(element.id().clone());
            }
            deleted.extend(subtree_of(element));
            grew = true;
        }
        if !grew {
            break;
        }
    }
    let dangling = dangling_references(doc, &subtree, &deleted);
    for (_, _, _, crosses) in &dangling {
        if *crosses && delete.cross_package_policy() == RejectIfExternalReferencesExist {
            return Err(Code::CMD_EXTERNAL_REFERENCES);
        }
        if policy == RejectIfReferenced {
            return Err(Code::CMD_DELETE_REFERENCED);
        }
    }
    // Check 4 (ADR 0039 D5): under RejectIfNonEmpty, a cascaded interface or
    // binding that contains an element other than a UtilityElement refuses.
    if delete.package_policy() == RejectIfNonEmpty {
        let guarded = doc
            .elements()
            .filter(|element| cascade_holders.contains(element.id()))
            .any(|holder| {
                subtree_of(holder)
                    .iter()
                    .filter(|id| *id != holder.id())
                    .filter_map(|id| doc.get(id))
                    .any(|element| !UTILITY_ELEMENTS.contains(&element.kind()))
            });
        if guarded {
            return Err(Code::CMD_PACKAGE_NOT_EMPTY);
        }
    }
    let changed = doc
        .elements()
        .filter(|element| !deleted.contains(element.id()))
        .filter(|element| {
            end_rules().any(|rule| {
                held_by(*element, rule)
                    .iter()
                    .any(|id| deleted.contains(id))
            })
        })
        .map(|element| element.id().clone())
        .collect();
    Ok(DeletePlan {
        deleted: doc
            .elements()
            .map(|element| element.id().clone())
            .filter(|id| deleted.contains(id))
            .collect(),
        changed,
        dangling,
    })
}

/// Every reference held by an element outside `subtree` whose id names an
/// element of `deleted`: by holder in pre-order, then end in the order of
/// `ReferenceEnd`, then position, one entry per distinct id and end. It
/// crosses when the holder's nearest package is not that of the element the
/// id names.
fn dangling_references(
    doc: &Document,
    subtree: &BTreeSet<ElementId>,
    deleted: &BTreeSet<ElementId>,
) -> Vec<Dangling> {
    let mut dangling = Vec::new();
    for holder in doc.elements() {
        if subtree.contains(holder.id()) {
            continue;
        }
        for rule in end_rules() {
            let mut seen = BTreeSet::new();
            for id in held_by(holder, rule) {
                if !deleted.contains(&id) || !seen.insert(id.clone()) {
                    continue;
                }
                let crosses = doc
                    .get(&id)
                    .is_some_and(|named| nearest_package(named) != nearest_package(holder));
                dangling.push((holder.id().clone(), rule.end, id, crosses));
            }
        }
    }
    dangling
}

/// The 12 deletes of `element`, one per policy combination, from the
/// strictest.
fn deletes_of(element: &ElementId) -> Vec<DeleteElement> {
    let mut deletes = Vec::new();
    for reference in REFERENCE_POLICIES {
        for package in PACKAGE_POLICIES {
            for cross in CROSS_PACKAGE_POLICIES {
                deletes.push(
                    DeleteElement::new(element)
                        .with_reference_policy(reference)
                        .with_package_policy(package)
                        .with_cross_package_policy(cross),
                );
            }
        }
    }
    deletes
}

/// The position of `delete`'s policies among the 12 combinations of
/// [`deletes_of`].
fn policy_index(delete: &DeleteElement) -> usize {
    let position = |found: Option<usize>| match found {
        Some(index) => index,
        None => panic!("{EDIT_005}: a policy this test does not know: {delete:?}"),
    };
    let reference = position(
        REFERENCE_POLICIES
            .iter()
            .position(|policy| *policy == delete.reference_policy()),
    );
    let package = position(
        PACKAGE_POLICIES
            .iter()
            .position(|policy| *policy == delete.package_policy()),
    );
    let cross = position(
        CROSS_PACKAGE_POLICIES
            .iter()
            .position(|policy| *policy == delete.cross_package_policy()),
    );
    (reference * PACKAGE_POLICIES.len() + package) * CROSS_PACKAGE_POLICIES.len() + cross
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
/// (`claim_1`, `argument_package_2`), an id that is not an `NCName` (`a b`),
/// held as inserted, an `AssertedInference` without ends, and, set with
/// `edit::apply`, a gid (` g `) and references (`loose_claim` cites
/// `claim_3`; `claim_1` has the meta-claim `c_asserted` twice).
fn built_with_insert() -> Document {
    let mut doc = inserted_elements();
    give_a_gid_and_references(&mut doc);
    doc
}

/// The elements of [`built_with_insert`], inserted.
fn inserted_elements() -> Document {
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
        WITH_SPACE,
        None,
    );
    insert(
        &mut doc,
        Some(WITH_SPACE),
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

/// Sets, with `edit::apply`, the gid ` g ` of `loose_claim`, its
/// `citedElement` `claim_3`, and the meta-claim `c_asserted`, twice, of
/// `claim_1`, in the document of [`inserted_elements`].
fn give_a_gid_and_references(doc: &mut Document) {
    setup_edit(EDIT_002, doc, SetGid::new("loose_claim", " g "));
    setup_edit(
        EDIT_002,
        doc,
        SetReferences::new("loose_claim", ReferenceEnd::CitedElement, ["claim_3"]),
    );
    setup_edit(
        EDIT_002,
        doc,
        SetReferences::new(
            "claim_1",
            ReferenceEnd::MetaClaim,
            ["c_asserted", "c_asserted"],
        ),
    );
}

/// Reads `input` with the default options.
fn read_text(what: &str, input: &[u8]) -> Document {
    match xmi::read(input, &ReadOptions::default()) {
        Ok(output) => output.into_document(),
        Err(error) => panic!("{EDIT_002}: setup: {what} does not read: {error}"),
    }
}

/// Reads the fixture `name` under [`FIXTURES`].
fn read_fixture(name: &str) -> Document {
    let path = Path::new(FIXTURES).join(name);
    let bytes = std::fs::read(&path)
        .unwrap_or_else(|error| panic!("{EDIT_002}: cannot read {}: {error}", path.display()));
    read_text(&format!("the fixture {name}"), &bytes)
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
        (
            "the edit start document".to_owned(),
            read_text("EDIT_START", EDIT_START.as_bytes()),
        ),
        (
            "the delete start document".to_owned(),
            read_text("DELETE_START", DELETE_START.as_bytes()),
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

/// The fixed list of operations for `doc`: the creates of
/// [`create_operations_for`], the edits of [`edit_operations_for`], the
/// moves of `move_operations_for` and the copies of `copy_operations_for`
/// (slice 0056), then the deletes of [`delete_operations_for`].
fn operations_for(doc: &Document) -> Vec<Operation> {
    let mut operations = create_operations_for(doc);
    operations.extend(edit_operations_for(doc));
    {
        operations.extend(move_operations_for(doc));
        operations.extend(copy_operations_for(doc));
    }
    operations.extend(delete_operations_for(doc));
    operations
}

/// The moves of the set (LIB-EDIT-017): of [`NOWHERE`], under and to the
/// roots; then for every element (pre-order): under the first and last
/// package of each family, the first element that is no package, and
/// [`NOWHERE`]; to position 0 of the first of those and past its end; into
/// itself and its first child; to the place it holds, to the end and the
/// start of its parent, and into its grandparent; and to the roots,
/// appended, first, at the place it holds and past the end.
fn move_operations_for(doc: &Document) -> Vec<Operation> {
    let mut operations: Vec<Operation> = vec![
        MoveElement::new(NOWHERE, NOWHERE).into(),
        MoveElement::root(NOWHERE).into(),
        MoveElement::root(NOWHERE).with_position(0).into(),
    ];
    let mut parents: Vec<ElementId> = Vec::new();
    for family in [
        ASSURANCE_CASE_PACKAGES,
        ARGUMENT_PACKAGES,
        ARTIFACT_PACKAGES,
    ] {
        if let Some((first, last)) = first_and_last_of(doc, &family) {
            if last != first {
                parents.push(last);
            }
            parents.push(first);
        }
    }
    parents.extend(first_not_of(doc, &PACKAGE_KINDS));
    parents.push(id(NOWHERE));
    let roots = doc.roots().count();
    for element in doc.elements() {
        let moved = element.id();
        for parent in &parents {
            operations.push(MoveElement::new(moved, parent).into());
        }
        if let Some(first) = parents.first() {
            let others = doc.get(first).map_or(0, |found| {
                found.children().filter(|child| child.id() != moved).count()
            });
            operations.push(MoveElement::new(moved, first).with_position(0).into());
            operations.push(
                MoveElement::new(moved, first)
                    .with_position(others + 1)
                    .into(),
            );
        }
        operations.push(MoveElement::new(moved, moved).into());
        if let Some(child) = element.children().next() {
            operations.push(MoveElement::new(moved, child.id()).into());
        }
        if let Some(parent) = element.parent() {
            let index = parent
                .children()
                .position(|child| child.id() == moved)
                .unwrap_or_default();
            operations.push(
                MoveElement::new(moved, parent.id())
                    .with_position(index)
                    .into(),
            );
            operations.push(MoveElement::new(moved, parent.id()).into());
            operations.push(MoveElement::new(moved, parent.id()).with_position(0).into());
            if let Some(grandparent) = parent.parent() {
                operations.push(MoveElement::new(moved, grandparent.id()).into());
            }
        } else {
            let index = doc
                .roots()
                .position(|root| root.id() == moved)
                .unwrap_or_default();
            operations.push(MoveElement::root(moved).with_position(index).into());
        }
        operations.push(MoveElement::root(moved).into());
        operations.push(MoveElement::root(moved).with_position(0).into());
        operations.push(MoveElement::root(moved).with_position(roots + 1).into());
    }
    operations
}

/// The fixed second source of the copies (LIB-EDIT-018), read as written:
/// in the ArgumentPackage `draft`, the `ArgumentReasoning` `r1`; the Claim
/// `top2` with a gid; the Claim `c1`, with an `xmi:id` on its name and a
/// Note, which clashes in several starting documents; the `AssertedInference`
/// `i1` from `c1` to `top2` with the reasoning `r1`; the Claim `out`, whose
/// `metaClaim` `goal` leaves the copy (a Claim of the delete start document,
/// absent elsewhere); the `AssertedContext` `ctx_bad` from `top2` to `case` (an
/// AssuranceCasePackage where a starting document has it); the
/// `AssertedInference` `nosrc`, with no source; and the nested ArgumentPackage
/// `sub`, holding the Claim `claim_1`.
const COPY_SOURCE: &str = r#"<sacm:ArgumentPackage xmlns:sacm="http://www.omg.org/spec/SACM/20220301" xmlns:xmi="http://www.omg.org/spec/XMI/20131001" xmi:id="draft">
  <name content="Draft"/>
  <argumentElement xmi:type="sacm:ArgumentReasoning" xmi:id="r1"><name content="R1"/></argumentElement>
  <argumentElement xmi:type="sacm:Claim" xmi:id="top2" gid="G-top"><name content="Top"/></argumentElement>
  <argumentElement xmi:type="sacm:Claim" xmi:id="c1"><name xmi:id="c1_name" lang="en" content="C1"/><note xmi:id="n9"/></argumentElement>
  <argumentElement xmi:type="sacm:AssertedInference" xmi:id="i1" source="c1" target="top2" reasoning="r1"><name content="I1"/></argumentElement>
  <argumentElement xmi:type="sacm:Claim" xmi:id="out" metaClaim="goal"><name content="Out"/></argumentElement>
  <argumentElement xmi:type="sacm:AssertedContext" xmi:id="ctx_bad" source="top2" target="case"><name content="Ctx"/></argumentElement>
  <argumentElement xmi:type="sacm:AssertedInference" xmi:id="nosrc" target="top2"><name content="No source"/></argumentElement>
  <argumentElement xmi:type="sacm:ArgumentPackage" xmi:id="sub">
    <name content="Sub"/>
    <argumentElement xmi:type="sacm:Claim" xmi:id="claim_1"><name content="Generated"/></argumentElement>
  </argumentElement>
</sacm:ArgumentPackage>
"#;

/// The copies of the set (LIB-EDIT-018). From [`COPY_SOURCE`], under the
/// first ArgumentPackage, the first ArtifactPackage, the first Claim and
/// [`NOWHERE`]: the draft (`r1`, `top2`, `c1`, `i1`), `out`, `ctx_bad` with
/// `top2`, `nosrc`, and nothing; and as roots `sub`, `top2` and the whole
/// draft. From `doc` itself: the second, third and last element of its
/// pre-order, each under its parent (or as a root), as a root and under
/// [`NOWHERE`]; the last element with its parent; the first ArgumentPackage
/// under its parent; and the first relationship with its sources.
fn copy_operations_for(doc: &Document) -> Vec<Operation> {
    let second = read_text("COPY_SOURCE", COPY_SOURCE.as_bytes());
    let list = |values: &[&str]| -> Vec<ElementId> { values.iter().copied().map(id).collect() };
    let mut operations = Vec::new();
    let mut parents: Vec<ElementId> = [
        first_of(doc, &ARGUMENT_PACKAGES),
        first_of(doc, &ARTIFACT_PACKAGES),
        first_of(doc, &[Claim]),
    ]
    .into_iter()
    .flatten()
    .collect();
    parents.push(id(NOWHERE));
    for parent in &parents {
        for values in [
            &["r1", "top2", "c1", "i1"][..],
            &["out"],
            &["ctx_bad", "top2"],
            &["nosrc"],
            &[],
        ] {
            operations.push(remembered_copy(&second, false, &list(values), Some(parent)));
        }
    }
    for values in [&["sub"][..], &["top2"], &["draft"]] {
        operations.push(remembered_copy(&second, false, &list(values), None));
    }
    let elements: Vec<SacmElement<'_>> = doc.elements().collect();
    let mut own: Vec<SacmElement<'_>> = elements.iter().skip(1).take(2).copied().collect();
    own.extend(elements.last().copied());
    for element in &own {
        let values = [element.id().clone()];
        let parent = element.parent().map(|parent| parent.id().clone());
        operations.push(remembered_copy(doc, true, &values, parent.as_ref()));
        operations.push(remembered_copy(doc, true, &values, None));
        operations.push(remembered_copy(doc, true, &values, Some(&id(NOWHERE))));
    }
    if let Some(last) = elements.last() {
        if let Some(parent) = last.parent() {
            let values = [last.id().clone(), parent.id().clone()];
            let holder = parent.parent().map(|holder| holder.id().clone());
            operations.push(remembered_copy(doc, true, &values, holder.as_ref()));
        }
    }
    if let Some(package) = first_of(doc, &ARGUMENT_PACKAGES).and_then(|found| doc.get(&found)) {
        let parent = package.parent().map(|parent| parent.id().clone());
        operations.push(remembered_copy(
            doc,
            true,
            &[package.id().clone()],
            parent.as_ref(),
        ));
    }
    if let Some(relationship) = elements
        .iter()
        .find(|element| RELATIONSHIPS.contains(&element.kind()) && !element.sources().is_empty())
    {
        let mut values = vec![relationship.id().clone()];
        values.extend(
            relationship
                .sources()
                .iter()
                .filter(|source| doc.contains(*source))
                .cloned(),
        );
        let parent = relationship.parent().map(|parent| parent.id().clone());
        operations.push(remembered_copy(doc, true, &values, parent.as_ref()));
    }
    operations
}

/// The 12 deletes of [`deletes_of`] of [`NOWHERE`], of the first id of `doc`
/// that identifies no element, and of every element of `doc` in reverse
/// pre-order, so that in sequence an element's contents go before it.
fn delete_operations_for(doc: &Document) -> Vec<Operation> {
    let mut targets = vec![id(NOWHERE)];
    targets.extend(first_id_of_no_element(doc));
    let mut elements: Vec<ElementId> = doc.elements().map(|element| element.id().clone()).collect();
    elements.reverse();
    targets.extend(elements);
    targets
        .iter()
        .flat_map(deletes_of)
        .map(Operation::from)
        .collect()
}

/// Under every element (pre-order) and under [`NOWHERE`]: one create of each
/// of the 20 kinds with each id form (the twelve of
/// [`CreateEnds::other_creates`] referencing ids of `doc`), then, with a
/// generated id, each reference refusal of [`reference_refusals`]; last both
/// root creates with each id form. Fresh caller ids are `new_<n>`, one per
/// parent, so that in sequence the second create given one is a duplicate.
fn create_operations_for(doc: &Document) -> Vec<Operation> {
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
    let ends = CreateEnds::of(doc);
    let interface_ends = InterfaceEnds::of(doc);

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
            let name = |kind: usize| name(parent_index, form, kind);
            let mut creates: Vec<Operation> = vec![
                CreateAssuranceCasePackage::new(parent, name(0)).into(),
                CreateArgumentPackage::new(parent, name(1)).into(),
                CreateClaim::new(parent, name(2)).into(),
            ];
            creates.extend(ends.other_creates(parent, &name, form));
            creates.extend(ends.artifact_creates(parent, &name));
            creates.extend(interface_ends.creates(parent, &name));
            operations.extend(
                creates
                    .into_iter()
                    .map(|create| with_caller_id(create, caller_id.as_ref())),
            );
        }
        let name = names[parent_index % names.len()].clone();
        operations.extend(reference_refusals(doc, parent, &name));
        operations.extend(interface_ends.refusals(doc, parent, &name));
    }
    let mut root_ids: Vec<Option<ElementId>> =
        vec![None, Some(id(&next_fresh())), Some(id(NOT_AN_NCNAME))];
    if let Some(first) = &first_id {
        root_ids.push(Some(first.clone()));
    }
    for (form, caller_id) in root_ids.into_iter().enumerate() {
        let case = CreateAssuranceCasePackage::root(name(parents.len(), form, 0));
        let arguments = CreateArgumentPackage::root(name(parents.len(), form, 1));
        let artifacts = CreateArtifactPackage::root(name(parents.len(), form, 2));
        // The interface and binding roots of slice 0066, built before the
        // caller id is moved.
        let interface_roots =
            interface_ends.roots(&|kind| name(parents.len(), form, kind), caller_id.as_ref());
        match caller_id {
            None => {
                operations.push(case.into());
                operations.push(arguments.into());
                operations.push(artifacts.into());
            }
            Some(value) => {
                operations.push(case.with_id(&value).into());
                operations.push(arguments.with_id(&value).into());
                operations.push(artifacts.with_id(&value).into());
            }
        }
        operations.extend(interface_roots);
    }
    operations
}

/// The first `xmi:id` of `doc` that identifies no element: of a name, a
/// `content`, a `key` or a `value`.
fn first_id_of_no_element(doc: &Document) -> Option<ElementId> {
    doc.elements()
        .find_map(|element| xmi_ids_of(element).get(1).copied().cloned())
}

/// The first and the last element of `doc`, in pre-order, of a kind in
/// `kinds`.
fn first_and_last_of(doc: &Document, kinds: &[ElementKind]) -> Option<(ElementId, ElementId)> {
    let mut matching = doc
        .elements()
        .filter(|element| kinds.contains(&element.kind()))
        .map(|element| element.id().clone());
    let first = matching.next()?;
    let last = matching.last().unwrap_or_else(|| first.clone());
    Some((first, last))
}

/// The first element of `doc`, in pre-order, of a kind not in `kinds`.
fn first_not_of(doc: &Document, kinds: &[ElementKind]) -> Option<ElementId> {
    doc.elements()
        .find(|element| !kinds.contains(&element.kind()))
        .map(|element| element.id().clone())
}

/// For every element (pre-order) and [`NOWHERE`]: the attribute, content,
/// reference and reorder edits the module docs list; then the root
/// reorders.
fn edit_operations_for(doc: &Document) -> Vec<Operation> {
    let names = names();
    let no_element = first_id_of_no_element(doc);
    let mut targets: Vec<ElementId> = doc.elements().map(|element| element.id().clone()).collect();
    targets.push(id(NOWHERE));
    let mut operations: Vec<Operation> = Vec::new();
    for (index, element) in targets.iter().enumerate() {
        let found = doc.get(element);
        // LIB-EDIT-008: name, gid, flags, declaration.
        operations.push(SetName::new(element, names[index % names.len()].clone()).into());
        if let Some(name) = found.and_then(SacmElement::name) {
            operations.push(SetName::new(element, name.clone()).into());
        }
        operations.push(SetGid::new(element, format!("g-{index}")).into());
        let gid = found.and_then(SacmElement::gid).unwrap_or("");
        operations.push(SetGid::new(element, gid).into());
        operations.push(SetGid::remove(element).into());
        for flag in FLAGS {
            for value in [true, false] {
                operations.push(SetFlag::new(element, flag, value).into());
            }
        }
        let declaration = AssertionDeclaration::ALL[index % AssertionDeclaration::ALL.len()];
        operations.push(SetAssertionDeclaration::new(element, declaration).into());
        if let Some(current) = found.and_then(SacmElement::assertion_declaration) {
            operations.push(SetAssertionDeclaration::new(element, current).into());
        }
        // LIB-EDIT-012: content and key.
        operations.push(SetContent::new(element, two_values()).into());
        if let Some(content) = found.and_then(SacmElement::content) {
            operations.push(SetContent::new(element, content.clone()).into());
        }
        operations.push(SetContent::remove(element).into());
        operations.push(SetKey::new(element, a_key()).into());
        if let Some(key) = found.and_then(SacmElement::key) {
            operations.push(SetKey::new(element, key.clone()).into());
        }
        // Slice 0055: each value of an ArtifactAsset, set, set to what it is,
        // and removed.
        push_asset_value_edits(&mut operations, found, element, index);
        // LIB-EDIT-009: every end.
        for rule in end_rules() {
            push_reference_edits(&mut operations, doc, element, rule, no_element.as_ref());
        }
        // LIB-EDIT-010: the children.
        let children: Vec<ElementId> = found
            .map(|found| found.children().map(|child| child.id().clone()).collect())
            .unwrap_or_default();
        let grandchild = found.and_then(|found| {
            found
                .children()
                .find_map(|child| child.children().next())
                .map(|grandchild| grandchild.id().clone())
        });
        for order in orders(&children, grandchild, no_element.as_ref()) {
            operations.push(ReorderElements::new(element, order).into());
        }
    }
    let roots: Vec<ElementId> = doc.roots().map(|root| root.id().clone()).collect();
    let not_a_root = doc
        .elements()
        .find(|element| element.parent().is_some())
        .map(|element| element.id().clone());
    for order in orders(&roots, not_a_root, no_element.as_ref()) {
        operations.push(ReorderElements::roots(order).into());
    }
    operations
}

/// The value edits of slice 0055 for `element` (`found` when the document
/// holds it, the `index`-th target): `version`, `date`, `startTime` and
/// `endTime` set to a new value, to the one held and removed; `location` set
/// to two values, to the one held and removed.
fn push_asset_value_edits(
    operations: &mut Vec<Operation>,
    found: Option<SacmElement<'_>>,
    element: &ElementId,
    index: usize,
) {
    operations.push(SetVersion::new(element, format!("v-{index}")).into());
    if let Some(version) = found.and_then(SacmElement::version) {
        operations.push(SetVersion::new(element, version).into());
    }
    operations.push(SetVersion::remove(element).into());
    operations.push(SetDate::new(element, format!("2024-03-{index:02}")).into());
    if let Some(date) = found.and_then(SacmElement::date) {
        operations.push(SetDate::new(element, date).into());
    }
    operations.push(SetDate::remove(element).into());
    operations.push(SetLocation::new(element, two_values()).into());
    if let Some(location) = found.and_then(SacmElement::location) {
        operations.push(SetLocation::new(element, location.clone()).into());
    }
    operations.push(SetLocation::remove(element).into());
    operations.push(SetStartTime::new(element, format!("s-{index}")).into());
    if let Some(start) = found.and_then(SacmElement::start_time) {
        operations.push(SetStartTime::new(element, start).into());
    }
    operations.push(SetStartTime::remove(element).into());
    operations.push(SetEndTime::new(element, format!("e-{index}")).into());
    if let Some(end) = found.and_then(SacmElement::end_time) {
        operations.push(SetEndTime::new(element, end).into());
    }
    operations.push(SetEndTime::remove(element).into());
}

/// The reorder lists for the children `children`: empty, reversed, as they
/// are, a repeated child, an id that is not a direct child (`not_a_child`),
/// the id of no element (`no_element`) and a missing id.
fn orders(
    children: &[ElementId],
    not_a_child: Option<ElementId>,
    no_element: Option<&ElementId>,
) -> Vec<Vec<ElementId>> {
    let mut orders = vec![Vec::new(), vec![id(NOWHERE)]];
    if let Some(first) = children.first() {
        orders.push(children.iter().rev().cloned().collect());
        orders.push(children.to_vec());
        orders.push(vec![first.clone(), first.clone()]);
    }
    orders.extend(not_a_child.map(|other| vec![other]));
    orders.extend(no_element.map(|other| vec![other.clone()]));
    orders
}

/// The reference edits of `rule`'s end on `element`.
fn push_reference_edits(
    operations: &mut Vec<Operation>,
    doc: &Document,
    element: &ElementId,
    rule: &EndRule,
    no_element: Option<&ElementId>,
) {
    let end = rule.end;
    let held_ids = doc
        .get(element)
        .map(|found| held(found, end))
        .unwrap_or_default();
    let well = first_and_last_of(doc, rule.declared);
    let wrong = first_not_of(doc, rule.declared);
    let mut lists: Vec<Vec<ElementId>> = vec![vec![id(NOWHERE)], vec![id(WITH_SPACE)]];
    let mut added: Vec<ElementId> = vec![id(NOWHERE), id(WITH_SPACE)];
    let mut removed: Vec<ElementId> = vec![id(NOWHERE)];
    if let Some((first, last)) = &well {
        lists.push(vec![first.clone()]);
        lists.push(vec![first.clone(), last.clone()]);
        added.push(first.clone());
        removed.push(first.clone());
    }
    if let Some(wrong) = &wrong {
        lists.push(vec![wrong.clone()]);
        added.push(wrong.clone());
    }
    if let Some(first_held) = held_ids.first() {
        lists.push(held_ids.clone());
        added.push(first_held.clone());
        removed.push(first_held.clone());
    }
    if let Some(no_element) = no_element {
        lists.push(vec![no_element.clone()]);
        added.push(no_element.clone());
    }
    operations.push(SetReferences::clear(element, end).into());
    for ids in lists {
        operations.push(SetReferences::new(element, end, ids).into());
    }
    for value in added {
        operations.push(AddReference::new(element, end, value).into());
    }
    for value in removed {
        operations.push(RemoveReference::new(element, end, value).into());
    }
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
fn for_every_application(check: impl FnMut(&Applied<'_>)) -> usize {
    for_selected_applications(|_| true, check)
}

/// As [`for_every_application`], but applies alone, and checks, only the
/// operations `selected` accepts. In sequence every operation is applied, so
/// that the state each one meets is the whole set's.
fn for_selected_applications(
    selected: impl Fn(&Operation) -> bool,
    mut check: impl FnMut(&Applied<'_>),
) -> usize {
    let mut count = 0;
    for (label, start) in starting_documents() {
        let operations = operations_for(&start);
        for (index, operation) in operations.iter().enumerate() {
            if !selected(operation) {
                continue;
            }
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
            if !selected(operation) {
                // Applied for the state it leaves; its answer is not checked.
                let _ = edit::apply(&mut doc, operation.clone());
                continue;
            }
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

/// Whether `operation` is an edit whose coverage label is in `labels`.
fn is_edit_in(operation: &Operation, labels: &[&str]) -> bool {
    matches!(classify(operation), Class::Edit { label } if labels.contains(&label))
}

/// Over the applications of the edits labelled `labels`: each answer is the
/// oracle's, and every label reaches every answer of [`required_answers`],
/// an `Ok` with empty `changed_ids` and one with an id, as
/// [`requires_unchanged_ok`] and [`requires_changed_ok`] ask.
fn assert_edit_answers_and_coverage(row: &str, labels: &[&str]) {
    let mut reached: BTreeSet<(&'static str, Result<(), &'static str>)> = BTreeSet::new();
    let mut unchanged: BTreeSet<&'static str> = BTreeSet::new();
    let mut changed: BTreeSet<&'static str> = BTreeSet::new();
    let total = for_selected_applications(
        |operation| is_edit_in(operation, labels),
        |applied| {
            let expected = expected_answer(applied.before, applied.operation);
            let actual = applied.answer.as_ref().map(|_| ()).map_err(EditError::code);
            assert_eq!(
                actual, expected,
                "{row}: {}: the answer is not the one this file's tables and ADR 0029 give",
                applied.context
            );
            let Class::Edit { label } = classify(applied.operation) else {
                return;
            };
            reached.insert((label, expected.map_err(Code::as_str)));
            if let Ok(mutation) = applied.answer {
                if mutation.changed_ids().is_empty() {
                    unchanged.insert(label);
                } else {
                    changed.insert(label);
                }
            }
        },
    );
    assert!(total > 0, "{row}: the set holds none of {labels:?}");
    for &label in labels {
        for answer in required_answers(label) {
            assert!(
                reached.contains(&(label, answer)),
                "{row}: the set never reaches {label} answering {answer:?}"
            );
        }
        if requires_unchanged_ok(label) {
            assert!(
                unchanged.contains(label),
                "{row}: the set never reaches {label} succeeding with empty changed_ids"
            );
        }
        if requires_changed_ok(label) {
            assert!(
                changed.contains(label),
                "{row}: the set never reaches {label} succeeding with a changed id"
            );
        }
    }
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
    let loose = inserted.get("loose_claim");
    assert_eq!(
        (
            loose.and_then(SacmElement::gid),
            loose
                .and_then(SacmElement::cited_element)
                .map(ElementId::as_str)
        ),
        (Some(" g "), Some("claim_3")),
        "{EDIT_002}: setup: the inserted document has no gid or reference"
    );
    let start = read_text("EDIT_START", EDIT_START.as_bytes());
    let get = |value: &str| match start.get(value) {
        Some(element) => element,
        None => panic!("{EDIT_002}: setup: the edit start document has no {value:?}"),
    };
    let kinds: BTreeSet<ElementKind> = start.elements().map(SacmElement::kind).collect();
    for kind in RELATIONSHIPS.into_iter().chain([
        ArtifactReference,
        ArgumentGroup,
        ArgumentReasoning,
        TaggedValue,
    ]) {
        assert!(
            kinds.contains(&kind),
            "{EDIT_002}: setup: the edit start document has no {kind}"
        );
    }
    assert!(
        !start.contains("x") && get("c1").meta_claims().contains(&id("x")),
        "{EDIT_002}: setup: the edit start document has no dangling id"
    );
    assert!(
        bad_references(&start)
            .iter()
            .any(|(_, _, target)| start.contains(target)),
        "{EDIT_002}: setup: the edit start document holds no wrongly typed id"
    );
    assert!(
        get("ac1").sources().is_empty(),
        "{EDIT_002}: setup: ac1 was read with a source"
    );
    assert!(
        get("case").name_id().is_some()
            && get("d1").content_id().is_some()
            && get("d1").content_value_ids().len() == 2
            && get("t1").key_id().is_some()
            && get("t1").key_value_ids().iter().all(Option::is_some),
        "{EDIT_002}: setup: the edit start document lacks the ids of a name, content or key"
    );
}

#[test]
fn lib_edit_002_the_set_has_legal_and_illegal_operations_of_every_kind_with_both_id_forms() {
    // (kind, caller id given, answer) for every create.
    let mut reached: BTreeSet<(ElementKind, bool, Result<(), &'static str>)> = BTreeSet::new();
    // (label, answer) for every edit.
    let mut reached_edits: BTreeSet<(&'static str, Result<(), &'static str>)> = BTreeSet::new();
    let total = for_every_application(|applied| {
        let expected = expected_answer(applied.before, applied.operation);
        let actual = applied.answer.as_ref().map(|_| ()).map_err(EditError::code);
        assert_eq!(
            actual, expected,
            "{EDIT_002}: {}: the answer is not the one the standard's table and ADR 0019/0029 give",
            applied.context
        );
        match classify(applied.operation) {
            Class::Create { kind, id, .. } => {
                reached.insert((kind, id.is_some(), expected.map_err(Code::as_str)));
            }
            Class::Edit { label } => {
                reached_edits.insert((label, expected.map_err(Code::as_str)));
            }
        }
    });
    assert!(total > 0, "{EDIT_002}: the set is empty");
    let mut required: Vec<(ElementKind, bool, Result<(), &'static str>)> = Vec::new();
    for kind in [
        AssuranceCasePackage,
        ArgumentPackage,
        Claim,
        ArgumentReasoning,
        ArtifactReference,
        ArgumentGroup,
        AssertedInference,
        AssertedContext,
        AssertedEvidence,
        AssertedArtifactSupport,
        AssertedArtifactContext,
        Description,
        ImplementationConstraint,
        Note,
        TaggedValue,
        ArtifactPackage,
        Property,
        Technique,
        Participant,
        ArtifactGroup,
        Artifact,
        Event,
        Resource,
        Activity,
        // The creates of slice 0066 (ADR 0039 D4).
        ArgumentPackageInterface,
        ArgumentPackageBinding,
        ArtifactPackageInterface,
        ArtifactPackageBinding,
    ] {
        for caller in [false, true] {
            required.push((kind, caller, Ok(())));
            required.push((kind, caller, Err(Code::CMD_TARGET_NOT_FOUND.as_str())));
            required.push((kind, caller, Err(Code::CMD_INVALID_PARENT.as_str())));
        }
        required.push((kind, true, Err(Code::ID_INVALID.as_str())));
        required.push((kind, true, Err(Code::CMD_DUPLICATE_ID.as_str())));
        // The checks of the ends (ADR 0027, ADR 0028), reached with a
        // generated id: a white-space id, and one naming a wrongly typed
        // element; a relationship with no source.
        // The artifact assets have no end of their own (§12.6). The
        // interfaces and bindings are packages with ends (ADR 0039).
        let interface_or_binding = [
            ArgumentPackageInterface,
            ArgumentPackageBinding,
            ArtifactPackageInterface,
            ArtifactPackageBinding,
        ]
        .contains(&kind);
        if (!UTILITY_ELEMENTS.contains(&kind)
            && !PACKAGE_KINDS.contains(&kind)
            && !CREATES_WITHOUT_ENDS.contains(&kind))
            || interface_or_binding
        {
            required.push((kind, false, Err(Code::ID_INVALID.as_str())));
            required.push((kind, false, Err(Code::REF_WRONG_TYPE.as_str())));
        }
        // A relationship with no source; a binding with fewer than two
        // distinct participants.
        if RELATIONSHIPS.contains(&kind)
            || [ArgumentPackageBinding, ArtifactPackageBinding].contains(&kind)
        {
            required.push((kind, false, Err(Code::MULTIPLICITY_VIOLATION.as_str())));
        }
    }
    for case in required {
        assert!(
            reached.contains(&case),
            "{EDIT_002}: the set never reaches (kind, caller id, answer) = {case:?}"
        );
    }
    for label in ATTRIBUTE_EDITS
        .into_iter()
        .chain(CONTENT_EDITS)
        .chain(REFERENCE_EDITS)
        .chain(REORDERS)
        .chain(DELETES)
        .chain(ASSET_VALUE_EDITS)
    {
        for answer in required_answers(label) {
            assert!(
                reached_edits.contains(&(label, answer)),
                "{EDIT_002}: the set never reaches {label} answering {answer:?}"
            );
        }
    }
}

#[test]
fn lib_edit_002_every_successful_operation_leaves_the_document_structurally_valid() {
    let mut successes = 0;
    for_every_application(|applied| {
        if applied.answer.is_ok() {
            successes += 1;
            assert_structurally_valid(EDIT_002, applied.context, applied.after);
            assert_edit_kept_structure(
                EDIT_002,
                applied.context,
                applied.operation,
                applied.before,
                applied.after,
            );
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

/// The [`Oracle`] of `operation` from `before` to `after`. `RootOrder` is
/// accepted only from a root reorder, and maps to no list: it is about the
/// document, not an element (ADR 0029).
fn oracle(context: &str, operation: &Operation, before: &Document, after: &Document) -> Oracle {
    let mut created = Vec::new();
    let mut deleted = Vec::new();
    let mut changed = BTreeSet::new();
    for difference in semantic_compare(before, after) {
        match &difference {
            Difference::RightOnly { id, .. } => created.push(id.clone()),
            Difference::LeftOnly { id, .. } => deleted.push(id.clone()),
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
            | Difference::EndTime { id, .. }
            // The ends of slice 0066 (ADR 0039 D1).
            | Difference::Interfaces { id, .. }
            | Difference::Implements { id, .. }
            | Difference::ParticipantPackages { id, .. } => {
                changed.insert(id.clone());
            }
            Difference::RootOrder { .. } => assert!(
                reorders_roots(operation),
                "{EDIT_003}: {context}: an operation that is no root reorder reordered the roots: \
                 {difference}"
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

/// Whether `operation` may change the order of the roots: a root reorder,
/// or a move to the roots (LIB-EDIT-017).
fn reorders_roots(operation: &Operation) -> bool {
    match operation {
        Operation::ReorderElements(reorder) => reorder.parent().is_none(),
        Operation::MoveElement(moved) => moved.parent().is_none(),
        _ => false,
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
        let expected = oracle(context, applied.operation, applied.before, applied.after);
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
        let Class::Create {
            kind,
            parent,
            id: caller_id,
        } = classify(applied.operation)
        else {
            return;
        };
        let context = applied.context;
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
        let Class::Create { id: given_id, .. } = classify(applied.operation) else {
            return;
        };
        let Class::Create { id: held_id, .. } = classify(mutation.operation()) else {
            panic!("{EDIT_003}: {context}: operation() holds an edit for a create");
        };
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
    let cases: [(Operation, &str); 21] = [
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
        (CreateClaim::new("p", name.clone()).into(), "CreateClaim"),
        (SetName::new("c1", name).into(), "SetName"),
        (SetGid::new("c1", "g").into(), "SetGid"),
        (SetGid::remove("c1").into(), "SetGid"),
        (SetFlag::new("c1", Flag::IsCitation, true).into(), "SetFlag"),
        (
            SetAssertionDeclaration::new("c1", AssertionDeclaration::Assumed).into(),
            "SetAssertionDeclaration",
        ),
        (SetContent::new("n1", two_values()).into(), "SetContent"),
        (SetContent::remove("n1").into(), "SetContent"),
        (SetKey::new("t1", a_key()).into(), "SetKey"),
        (
            SetReferences::new("i1", ReferenceEnd::Source, ["c2"]).into(),
            "SetReferences",
        ),
        (
            SetReferences::clear("i1", ReferenceEnd::Reasoning).into(),
            "SetReferences",
        ),
        (
            AddReference::new("i1", ReferenceEnd::Source, "c3").into(),
            "AddReference",
        ),
        (
            RemoveReference::new("i1", ReferenceEnd::Source, "c3").into(),
            "RemoveReference",
        ),
        (
            ReorderElements::new("p", ["c2", "c1"]).into(),
            "ReorderElements",
        ),
        (ReorderElements::roots(["q", "p"]).into(), "ReorderElements"),
        (SetFlag::new("i1", Flag::IsCounter, false).into(), "SetFlag"),
        (
            SetReferences::new("c1", ReferenceEnd::GroupMember, Vec::<ElementId>::new()).into(),
            "SetReferences",
        ),
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
fn lib_edit_003_operation_name_of_the_artifact_creates_is_the_struct_name() {
    // The creates of slice 0052 and an edit of its end (ADR 0034).
    let cases: [(Operation, &str); 7] = [
        (
            CreateArtifactPackage::root(named("A")).into(),
            "CreateArtifactPackage",
        ),
        (
            CreateArtifactPackage::new("case", named("A"))
                .with_id("x")
                .into(),
            "CreateArtifactPackage",
        ),
        (
            CreateProperty::new("t1", named("P")).into(),
            "CreateProperty",
        ),
        (
            CreateTechnique::new("a", named("T")).into(),
            "CreateTechnique",
        ),
        (
            CreateParticipant::new("a", named("P")).into(),
            "CreateParticipant",
        ),
        (
            CreateArtifactGroup::new("a", named("G"))
                .with_group_members(["t1"])
                .into(),
            "CreateArtifactGroup",
        ),
        (
            AddReference::new("g1", ReferenceEnd::ArtifactGroupMember, "t1").into(),
            "AddReference",
        ),
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
    // An edit's mutation holds the edit, and its name.
    let edits: [(Operation, &str); 2] = [
        (SetGid::new("claim_1", "g").into(), "SetGid"),
        (ReorderElements::roots(["case"]).into(), "ReorderElements"),
    ];
    for (operation, expected) in edits {
        let mutation = match edit::apply(&mut doc, operation.clone()) {
            Ok(mutation) => mutation,
            Err(error) => panic!("{EDIT_003}: setup: {operation:?} was refused: {error}"),
        };
        assert_eq!(
            mutation.operation().operation_name(),
            expected,
            "{EDIT_003}: the name of the edit held by the mutation"
        );
    }
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

// ---------------------------------------------------------------------------
// LIB-EDIT-008, -009, -010, -012: the set operations join the set
// ---------------------------------------------------------------------------

#[test]
fn lib_edit_008_the_attribute_edits_join_the_set_with_every_answer() {
    assert_edit_answers_and_coverage(EDIT_008, &ATTRIBUTE_EDITS);
}

#[test]
fn lib_edit_008_no_successful_operation_of_the_set_removes_a_name() {
    let mut successes = 0;
    for_every_application(|applied| {
        if applied.answer.is_ok() {
            successes += 1;
            assert_names_kept(EDIT_008, applied.context, applied.before, applied.after);
        }
    });
    assert!(
        successes > 0,
        "{EDIT_008}: no operation of the set succeeded"
    );
}

#[test]
fn lib_edit_009_the_reference_edits_join_the_set_with_every_answer() {
    assert_edit_answers_and_coverage(EDIT_009, &REFERENCE_EDITS);
}

#[test]
fn lib_edit_009_no_successful_edit_of_the_set_adds_a_bad_reference_or_empties_a_required_end() {
    let mut successes = 0;
    for_selected_applications(
        |operation| is_edit_in(operation, &REFERENCE_EDITS),
        |applied| {
            if applied.answer.is_ok() {
                successes += 1;
                assert_no_new_bad_references(
                    EDIT_009,
                    applied.context,
                    applied.before,
                    applied.after,
                );
                assert_required_ends_kept(EDIT_009, applied.context, applied.before, applied.after);
            }
        },
    );
    assert!(
        successes > 0,
        "{EDIT_009}: no reference edit of the set succeeded"
    );
}

#[test]
fn lib_edit_010_reorder_joins_the_set_with_every_answer() {
    assert_edit_answers_and_coverage(EDIT_010, &REORDERS);
}

#[test]
fn lib_edit_010_every_successful_reorder_of_the_set_keeps_the_elements_parents_and_findings() {
    let mut root_orders = 0;
    for_selected_applications(
        |operation| is_edit_in(operation, &REORDERS),
        |applied| {
            if applied.answer.is_err() {
                return;
            }
            let context = applied.context;
            let (before, after) = (applied.before, applied.after);
            assert_eq!(
                after.element_count(),
                before.element_count(),
                "{EDIT_010}: {context}: the reorder changed the element count"
            );
            for element in before.elements() {
                let now = after.get(element.id());
                assert_eq!(
                    now.and_then(SacmElement::parent).map(SacmElement::id),
                    element.parent().map(SacmElement::id),
                    "{EDIT_010}: {context}: the reorder moved {:?}",
                    element.id()
                );
            }
            let differences = semantic_compare(before, after);
            assert!(
                differences.iter().all(|difference| matches!(
                    difference,
                    Difference::ChildOrder { .. } | Difference::RootOrder { .. }
                )),
                "{EDIT_010}: {context}: the reorder changed more than an order: {differences:?}"
            );
            if differences
                .iter()
                .any(|difference| matches!(difference, Difference::RootOrder { .. }))
            {
                root_orders += 1;
            }
        },
    );
    assert!(
        root_orders > 0,
        "{EDIT_010}: no root reorder of the set changed the root order"
    );
}

#[test]
fn lib_edit_012_the_content_edits_join_the_set_with_every_answer() {
    assert_edit_answers_and_coverage(EDIT_012, &CONTENT_EDITS);
}

#[test]
fn lib_edit_012_no_successful_operation_of_the_set_removes_a_key() {
    let mut successes = 0;
    for_every_application(|applied| {
        if applied.answer.is_ok() {
            successes += 1;
            assert_keys_kept(EDIT_012, applied.context, applied.before, applied.after);
        }
    });
    assert!(
        successes > 0,
        "{EDIT_012}: no operation of the set succeeded"
    );
}

// ---------------------------------------------------------------------------
// LIB-EDIT-013: check an operation without applying it
// ---------------------------------------------------------------------------

const OPERATION_NAME_COUNT: usize = 46;
const OPERATION_NAMES: [&str; OPERATION_NAME_COUNT] = [
    "CreateAssuranceCasePackage",
    "CreateArgumentPackage",
    "CreateClaim",
    "CreateArgumentReasoning",
    "CreateAssertedInference",
    "CreateAssertedContext",
    "CreateArtifactReference",
    "CreateArgumentGroup",
    "CreateAssertedEvidence",
    "CreateAssertedArtifactSupport",
    "CreateAssertedArtifactContext",
    "CreateDescription",
    "CreateImplementationConstraint",
    "CreateNote",
    "CreateTaggedValue",
    "CreateArtifactPackage",
    "CreateProperty",
    "CreateTechnique",
    "CreateParticipant",
    "CreateArtifactGroup",
    "CreateArtifact",
    "CreateEvent",
    "CreateResource",
    "CreateActivity",
    // The creates of slice 0066 (SACM23-ARG-014, -015, SACM23-ART-012, -013).
    "CreateArgumentPackageInterface",
    "CreateArgumentPackageBinding",
    "CreateArtifactPackageInterface",
    "CreateArtifactPackageBinding",
    "SetName",
    "SetGid",
    "SetFlag",
    "SetAssertionDeclaration",
    "SetContent",
    "SetKey",
    "SetVersion",
    "SetDate",
    "SetLocation",
    "SetStartTime",
    "SetEndTime",
    "SetReferences",
    "AddReference",
    "RemoveReference",
    "ReorderElements",
    "MoveElement",
    "CopyElements",
    "DeleteElement",
];

const CREATE_COUNT: usize = 28;

/// A document read from XMI for the check-order cases: a Claim `c1` with a
/// Description `d1`, a Claim with the id `a b` (not an `NCName`, kept as
/// read), an `ArgumentReasoning` `why` and an `AssertedInference` `i1` from `c1`
/// to `c1`, in the ArgumentPackage `p` of the AssuranceCasePackage `case`;
/// and the empty ArtifactPackage `evidence` there, for the artifact creates.
const CHECK_ORDER_START: &str = r#"<sacm:AssuranceCasePackage xmlns:sacm="http://www.omg.org/spec/SACM/20220301" xmlns:xmi="http://www.omg.org/spec/XMI/20131001" xmi:id="case">
  <name content="Case"/>
  <argumentPackage xmi:id="p">
    <name content="P"/>
    <argumentElement xmi:type="sacm:Claim" xmi:id="c1">
      <name content="C1"/>
      <description xmi:id="d1"><content><value content="D1"/></content></description>
    </argumentElement>
    <argumentElement xmi:type="sacm:Claim" xmi:id="a b"><name content="Spaced"/></argumentElement>
    <argumentElement xmi:type="sacm:ArgumentReasoning" xmi:id="why"><name content="Why"/></argumentElement>
    <argumentElement xmi:type="sacm:AssertedInference" xmi:id="i1" source="c1" target="c1"><name content="I1"/></argumentElement>
  </argumentPackage>
  <artifactPackage xmi:id="evidence"><name content="Evidence"/></artifactPackage>
</sacm:AssuranceCasePackage>
"#;

/// [`CHECK_ORDER_START`] with a second ArgumentPackage `q` and a second
/// ArtifactPackage `evidence2` at the end of `case`, which the bindings of
/// [`interface_creates_of_every_kind`] bind (slice 0066).
fn check_order_start() -> String {
    CHECK_ORDER_START.replacen(
        "</sacm:AssuranceCasePackage>",
        "  <argumentPackage xmi:id=\"q\"><name content=\"Q\"/></argumentPackage>\n  \
         <artifactPackage xmi:id=\"evidence2\"><name content=\"Evidence 2\"/></artifactPackage>\n\
         </sacm:AssuranceCasePackage>",
        1,
    )
}

/// Whether a create has a caller id: `Some(true)` for a caller id,
/// `Some(false)` for a generated one, and `None` for an operation that is
/// not a create.
fn has_caller_id(operation: &Operation) -> Option<bool> {
    let id = match operation {
        Operation::CreateAssuranceCasePackage(create) => create.id(),
        Operation::CreateArgumentPackage(create) => create.id(),
        Operation::CreateClaim(create) => create.id(),
        Operation::CreateArgumentReasoning(create) => create.id(),
        Operation::CreateAssertedInference(create) => create.id(),
        Operation::CreateAssertedContext(create) => create.id(),
        Operation::CreateArtifactReference(create) => create.id(),
        Operation::CreateArgumentGroup(create) => create.id(),
        Operation::CreateAssertedEvidence(create) => create.id(),
        Operation::CreateAssertedArtifactSupport(create) => create.id(),
        Operation::CreateAssertedArtifactContext(create) => create.id(),
        Operation::CreateDescription(create) => create.id(),
        Operation::CreateImplementationConstraint(create) => create.id(),
        Operation::CreateNote(create) => create.id(),
        Operation::CreateTaggedValue(create) => create.id(),
        Operation::CreateArtifactPackage(create) => create.id(),
        Operation::CreateProperty(create) => create.id(),
        Operation::CreateTechnique(create) => create.id(),
        Operation::CreateParticipant(create) => create.id(),
        Operation::CreateArtifactGroup(create) => create.id(),
        Operation::CreateArtifact(create) => create.id(),
        Operation::CreateEvent(create) => create.id(),
        Operation::CreateResource(create) => create.id(),
        Operation::CreateActivity(create) => create.id(),
        Operation::CreateArgumentPackageInterface(create) => create.id(),
        Operation::CreateArgumentPackageBinding(create) => create.id(),
        Operation::CreateArtifactPackageInterface(create) => create.id(),
        Operation::CreateArtifactPackageBinding(create) => create.id(),
        _ => return None,
    };
    Some(id.is_some())
}

/// `operation`, a create, given the caller id `value`; unchanged for `None`.
fn with_caller_id(operation: Operation, value: Option<&ElementId>) -> Operation {
    let Some(value) = value else {
        return operation;
    };
    match operation {
        Operation::CreateAssuranceCasePackage(create) => create.with_id(value).into(),
        Operation::CreateArgumentPackage(create) => create.with_id(value).into(),
        Operation::CreateClaim(create) => create.with_id(value).into(),
        Operation::CreateArgumentReasoning(create) => create.with_id(value).into(),
        Operation::CreateAssertedInference(create) => create.with_id(value).into(),
        Operation::CreateAssertedContext(create) => create.with_id(value).into(),
        Operation::CreateArtifactReference(create) => create.with_id(value).into(),
        Operation::CreateArgumentGroup(create) => create.with_id(value).into(),
        Operation::CreateAssertedEvidence(create) => create.with_id(value).into(),
        Operation::CreateAssertedArtifactSupport(create) => create.with_id(value).into(),
        Operation::CreateAssertedArtifactContext(create) => create.with_id(value).into(),
        Operation::CreateDescription(create) => create.with_id(value).into(),
        Operation::CreateImplementationConstraint(create) => create.with_id(value).into(),
        Operation::CreateNote(create) => create.with_id(value).into(),
        Operation::CreateTaggedValue(create) => create.with_id(value).into(),
        Operation::CreateArtifactPackage(create) => create.with_id(value).into(),
        Operation::CreateProperty(create) => create.with_id(value).into(),
        Operation::CreateTechnique(create) => create.with_id(value).into(),
        Operation::CreateParticipant(create) => create.with_id(value).into(),
        Operation::CreateArtifactGroup(create) => create.with_id(value).into(),
        Operation::CreateArtifact(create) => create.with_id(value).into(),
        Operation::CreateEvent(create) => create.with_id(value).into(),
        Operation::CreateResource(create) => create.with_id(value).into(),
        Operation::CreateActivity(create) => create.with_id(value).into(),
        Operation::CreateArgumentPackageInterface(create) => create.with_id(value).into(),
        Operation::CreateArgumentPackageBinding(create) => create.with_id(value).into(),
        Operation::CreateArtifactPackageInterface(create) => create.with_id(value).into(),
        Operation::CreateArtifactPackageBinding(create) => create.with_id(value).into(),
        other => panic!("{EDIT_002}: setup: {other:?} is not a create"),
    }
}

/// The five asserted relationship creates under `parent`, from `sources` to
/// `target`, with `reasoning` if given, and `is_counter`.
fn relationship_creates(
    parent: &ElementId,
    name: &LangString,
    sources: &[ElementId],
    target: &ElementId,
    reasoning: Option<&ElementId>,
    is_counter: bool,
) -> [Operation; 5] {
    macro_rules! relationship {
        ($create:ident) => {{
            let mut create =
                $create::new(parent, name.clone(), sources, target).with_is_counter(is_counter);
            if let Some(reasoning) = reasoning {
                create = create.with_reasoning(reasoning);
            }
            Operation::from(create)
        }};
    }
    [
        relationship!(CreateAssertedInference),
        relationship!(CreateAssertedContext),
        relationship!(CreateAssertedEvidence),
        relationship!(CreateAssertedArtifactSupport),
        relationship!(CreateAssertedArtifactContext),
    ]
}

/// The first element of `doc`, in pre-order, of a kind in `kinds`.
fn first_of(doc: &Document, kinds: &[ElementKind]) -> Option<ElementId> {
    first_and_last_of(doc, kinds).map(|(first, _)| first)
}

/// The ids the twelve creates that are no package and no Claim reference in
/// `doc`: the first element of each end's declared type where the document
/// has one, and [`NOWHERE`] for a source or target where it has none.
/// The ids the four creates of slice 0066 reference in `doc` (ADR 0039 D4):
/// the first `ArgumentPackage` of any kind (or [`NOWHERE`]) as `implements`,
/// and the first and last as participants (one id twice when the document
/// has one, which a binding refuses); the same for the `ArtifactPackage`s.
struct InterfaceEnds {
    argument: (ElementId, ElementId),
    artifact: (ElementId, ElementId),
}

impl InterfaceEnds {
    fn of(doc: &Document) -> Self {
        let pair = |kinds: &[ElementKind]| {
            first_and_last_of(doc, kinds).unwrap_or_else(|| (id(NOWHERE), id(NOWHERE)))
        };
        Self {
            argument: pair(&ARGUMENT_PACKAGES),
            artifact: pair(&ARTIFACT_PACKAGES),
        }
    }

    /// The four creates under `parent`, with generated ids.
    fn creates(&self, parent: &ElementId, name: &dyn Fn(usize) -> LangString) -> Vec<Operation> {
        let (argument, last_argument) = &self.argument;
        let (artifact, last_artifact) = &self.artifact;
        vec![
            CreateArgumentPackageInterface::new(parent, name(17), argument).into(),
            CreateArgumentPackageBinding::new(parent, name(18), [argument, last_argument]).into(),
            CreateArtifactPackageInterface::new(parent, name(19), artifact).into(),
            CreateArtifactPackageBinding::new(parent, name(20), [artifact, last_artifact]).into(),
        ]
    }

    /// The four creates as roots, with the caller id `caller_id` if given.
    fn roots(
        &self,
        name: &dyn Fn(usize) -> LangString,
        caller_id: Option<&ElementId>,
    ) -> Vec<Operation> {
        let (argument, last_argument) = &self.argument;
        let (artifact, last_artifact) = &self.artifact;
        let creates: Vec<Operation> = vec![
            CreateArgumentPackageInterface::root(name(3), argument).into(),
            CreateArgumentPackageBinding::root(name(4), [argument, last_argument]).into(),
            CreateArtifactPackageInterface::root(name(5), artifact).into(),
            CreateArtifactPackageBinding::root(name(6), [artifact, last_artifact]).into(),
        ];
        creates
            .into_iter()
            .map(|create| with_caller_id(create, caller_id))
            .collect()
    }

    /// The reference refusals of the four creates under `parent`, with
    /// generated ids: an `implements` or a participant that holds white
    /// space, names no element, or names the first element of a kind its end
    /// does not admit, each participant with a second, distinct one; and a
    /// binding with no participant.
    fn refusals(&self, doc: &Document, parent: &ElementId, name: &LangString) -> Vec<Operation> {
        let bad = |declared: &[ElementKind]| -> Vec<ElementId> {
            let mut ids = vec![id(WITH_SPACE), id(NOWHERE)];
            ids.extend(first_not_of(doc, declared));
            ids
        };
        let mut operations: Vec<Operation> = vec![
            CreateArgumentPackageBinding::new(parent, name.clone(), Vec::<ElementId>::new()).into(),
            CreateArtifactPackageBinding::new(parent, name.clone(), Vec::<ElementId>::new()).into(),
        ];
        for value in bad(&ARGUMENT_PACKAGES) {
            operations.push(
                CreateArgumentPackageInterface::new(parent, name.clone(), value.clone()).into(),
            );
            operations.push(
                CreateArgumentPackageBinding::new(parent, name.clone(), [&value, &self.argument.0])
                    .into(),
            );
        }
        for value in bad(&ARTIFACT_PACKAGES) {
            operations.push(
                CreateArtifactPackageInterface::new(parent, name.clone(), value.clone()).into(),
            );
            operations.push(
                CreateArtifactPackageBinding::new(parent, name.clone(), [&value, &self.artifact.0])
                    .into(),
            );
        }
        operations
    }
}

struct CreateEnds {
    asset: ElementId,
    reasoning: Option<ElementId>,
    structure: Option<ElementId>,
    artifact: Option<ElementId>,
    member: Option<ElementId>,
    artifact_member: Option<ElementId>,
}

impl CreateEnds {
    fn of(doc: &Document) -> Self {
        Self {
            asset: first_of(doc, &ARGUMENT_ASSETS).unwrap_or_else(|| id(NOWHERE)),
            reasoning: first_of(doc, &[ArgumentReasoning]),
            structure: first_of(doc, &[ArgumentPackage]),
            artifact: first_of(doc, &MODEL_ELEMENTS),
            member: first_of(doc, &ARGUMENTATION_ELEMENTS),
            artifact_member: first_of(doc, &MODEL_ELEMENTS),
        }
    }

    /// The five creates of slice 0052 under `parent`, with generated ids,
    /// named by `name`: an `ArtifactPackage`, a Property, a Technique, a
    /// Participant, and an `ArtifactGroup` with the member `artifact_member`.
    fn artifact_creates(
        &self,
        parent: &ElementId,
        name: &dyn Fn(usize) -> LangString,
    ) -> Vec<Operation> {
        let mut group = CreateArtifactGroup::new(parent, name(12));
        if let Some(member) = &self.artifact_member {
            group = group.with_group_members([member]);
        }
        vec![
            CreateArtifactPackage::new(parent, name(8)).into(),
            CreateProperty::new(parent, name(9)).into(),
            CreateTechnique::new(parent, name(10)).into(),
            CreateParticipant::new(parent, name(11)).into(),
            group.into(),
            // The creates of slice 0055, each with its values.
            CreateArtifact::new(parent, name(13))
                .with_version("1.2")
                .with_date("2024-03-01")
                .into(),
            CreateEvent::new(parent, name(14))
                .with_date("spring 2024")
                .into(),
            CreateResource::new(parent, name(15))
                .with_location(two_values())
                .into(),
            CreateActivity::new(parent, name(16))
                .with_start_time("2024-02-12")
                .with_end_time("2024-02-01")
                .into(),
        ]
    }

    /// The twelve creates that are no package and no Claim under `parent`,
    /// with generated ids, named by `name`: the `ArgumentReasoning`,
    /// `ArtifactReference` and `ArgumentGroup` with these ends, the five asserted
    /// relationships from and to `asset` with `reasoning` (`isCounter` for an
    /// odd `form`), and the four `UtilityElement`s.
    fn other_creates(
        &self,
        parent: &ElementId,
        name: &dyn Fn(usize) -> LangString,
        form: usize,
    ) -> Vec<Operation> {
        let mut argument_reasoning = CreateArgumentReasoning::new(parent, name(3));
        if let Some(structure) = &self.structure {
            argument_reasoning = argument_reasoning.with_structure(structure);
        }
        let mut reference = CreateArtifactReference::new(parent, name(4));
        if let Some(artifact) = &self.artifact {
            reference = reference.with_referenced_artifact_elements([artifact]);
        }
        let mut group = CreateArgumentGroup::new(parent, name(5));
        if let Some(member) = &self.member {
            group = group.with_group_members([member]);
        }
        let mut creates: Vec<Operation> =
            vec![argument_reasoning.into(), reference.into(), group.into()];
        creates.extend(relationship_creates(
            parent,
            &name(6),
            std::slice::from_ref(&self.asset),
            &self.asset,
            self.reasoning.as_ref(),
            form % 2 == 1,
        ));
        creates.push(CreateDescription::new(parent, two_values()).into());
        creates.push(CreateImplementationConstraint::new(parent, name(7)).into());
        creates.push(CreateNote::new(parent, MultiLangString::default()).into());
        creates.push(CreateTaggedValue::new(parent, a_key(), two_values()).into());
        creates
    }
}

/// The reference refusals under `parent`, with generated ids: an asserted
/// relationship of each kind with no source; with a source, a target or a
/// reasoning that holds white space ([`WITH_SPACE`]), names no element
/// ([`NOWHERE`]), or names the first element of a kind its end does not
/// admit; and an `ArgumentReasoning` `structure`, an `ArtifactReference`
/// `referencedArtifactElement`, an `ArgumentGroup` member and an
/// `ArtifactGroup` member of each of those three.
fn reference_refusals(doc: &Document, parent: &ElementId, name: &LangString) -> Vec<Operation> {
    let asset = first_of(doc, &ARGUMENT_ASSETS).unwrap_or_else(|| id(NOWHERE));
    let bad = |declared: &[ElementKind]| -> Vec<ElementId> {
        let mut ids = vec![id(WITH_SPACE), id(NOWHERE)];
        ids.extend(first_not_of(doc, declared));
        ids
    };
    let mut operations: Vec<Operation> = Vec::new();
    operations.extend(relationship_creates(parent, name, &[], &asset, None, false));
    for value in bad(&ARGUMENT_ASSETS) {
        operations.extend(relationship_creates(
            parent,
            name,
            std::slice::from_ref(&value),
            &asset,
            None,
            false,
        ));
        operations.extend(relationship_creates(
            parent,
            name,
            std::slice::from_ref(&asset),
            &value,
            None,
            false,
        ));
    }
    for value in bad(&[ArgumentReasoning]) {
        operations.extend(relationship_creates(
            parent,
            name,
            std::slice::from_ref(&asset),
            &asset,
            Some(&value),
            false,
        ));
    }
    for value in bad(&[ArgumentPackage]) {
        operations.push(
            CreateArgumentReasoning::new(parent, name.clone())
                .with_structure(value)
                .into(),
        );
    }
    for value in bad(&MODEL_ELEMENTS) {
        operations.push(
            CreateArtifactReference::new(parent, name.clone())
                .with_referenced_artifact_elements([value])
                .into(),
        );
    }
    for value in bad(&ARGUMENTATION_ELEMENTS) {
        operations.push(
            CreateArgumentGroup::new(parent, name.clone())
                .with_group_members([value])
                .into(),
        );
    }
    for value in bad(&MODEL_ELEMENTS) {
        operations.push(
            CreateArtifactGroup::new(parent, name.clone())
                .with_group_members([value])
                .into(),
        );
    }
    operations
}

/// The LIB-EDIT-013 set for `doc`: the operations of [`operations_for`],
/// whose creates include every create with both id forms and its reference
/// refusals.
fn check_operations_for(doc: &Document) -> Vec<Operation> {
    operations_for(doc)
}

/// One create of each of the 20 kinds, with a generated id, in the order of
/// [`probe_index`]: the packages under `package_parent`, the argumentation
/// and utility kinds under `parent`, the asserted relationships from and to
/// `asset`, then [`artifact_creates_of_every_kind`] with `artifact_parent`.
fn creates_of_every_kind(
    package_parent: &str,
    parent: &str,
    asset: &str,
    artifact_parent: &str,
) -> Vec<Operation> {
    let mut creates = argument_creates_of_every_kind(package_parent, parent, asset);
    creates.extend(artifact_creates_of_every_kind(
        package_parent,
        artifact_parent,
    ));
    creates.extend(interface_creates_of_every_kind(
        package_parent,
        parent,
        artifact_parent,
    ));
    creates
}

/// The five creates of slice 0052, with a generated id: the `ArtifactPackage`
/// under `package_parent`, then a Property, a Technique, a Participant and an
/// `ArtifactGroup` under `artifact_parent`.
fn artifact_creates_of_every_kind(package_parent: &str, artifact_parent: &str) -> Vec<Operation> {
    vec![
        CreateArtifactPackage::new(package_parent, named("Artifacts")).into(),
        CreateProperty::new(artifact_parent, named("Property")).into(),
        CreateTechnique::new(artifact_parent, named("Technique")).into(),
        CreateParticipant::new(artifact_parent, named("Participant")).into(),
        CreateArtifactGroup::new(artifact_parent, named("Artifact group")).into(),
        // The creates of slice 0055, in the order of `Operation`.
        CreateArtifact::new(artifact_parent, named("Artifact")).into(),
        CreateEvent::new(artifact_parent, named("Event")).into(),
        CreateResource::new(artifact_parent, named("Resource")).into(),
        CreateActivity::new(artifact_parent, named("Activity")).into(),
    ]
}

/// The first 15 of [`creates_of_every_kind`].
fn argument_creates_of_every_kind(
    package_parent: &str,
    parent: &str,
    asset: &str,
) -> Vec<Operation> {
    let parent = id(parent);
    let asset = id(asset);
    let mut creates: Vec<Operation> = vec![
        CreateAssuranceCasePackage::new(package_parent, named("Case")).into(),
        CreateArgumentPackage::new(package_parent, named("Arguments")).into(),
        CreateClaim::new(&parent, named("Claim")).into(),
        CreateArgumentReasoning::new(&parent, named("Reasoning")).into(),
        CreateArtifactReference::new(&parent, named("Reference")).into(),
        CreateArgumentGroup::new(&parent, named("Group")).into(),
    ];
    creates.extend(relationship_creates(
        &parent,
        &named("Relationship"),
        std::slice::from_ref(&asset),
        &asset,
        None,
        false,
    ));
    creates.push(CreateDescription::new(&parent, named("Description")).into());
    creates.push(CreateImplementationConstraint::new(&parent, named("Constraint")).into());
    creates.push(CreateNote::new(&parent, named("Note")).into());
    creates.push(CreateTaggedValue::new(&parent, a_key(), named("Value")).into());
    creates
}

/// The ids the next generated element of each of the 20 kinds the edit API
/// creates would get, observed on a clone so that `doc` is untouched: a root
/// AssuranceCasePackage and ArgumentPackage, a Claim in that ArgumentPackage,
/// and there one of each other argumentation and utility kind, the
/// relationships from and to the Claim; then an ArtifactPackage in the
/// AssuranceCasePackage, and in it one of each other artifact kind.
fn next_generated_ids_of_every_kind(row: &str, doc: &Document) -> Vec<ElementId> {
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
    let mut ids = vec![case, arguments.clone(), claim.clone()];
    for operation in
        argument_creates_of_every_kind(arguments.as_str(), arguments.as_str(), claim.as_str())
            .into_iter()
            .skip(3)
    {
        ids.push(create(row, &mut probe, operation));
    }
    let mut artifact_creates = artifact_creates_of_every_kind(ids[0].as_str(), "").into_iter();
    let Some(package) = artifact_creates.next() else {
        panic!("{row}: setup: no ArtifactPackage create");
    };
    let artifacts = create(row, &mut probe, package);
    ids.push(artifacts.clone());
    for operation in artifact_creates_of_every_kind(ids[0].as_str(), artifacts.as_str())
        .into_iter()
        .skip(1)
    {
        ids.push(create(row, &mut probe, operation));
    }
    // The creates of slice 0066: an interface of each package, and a binding
    // of the package and its interface.
    {
        let interface = create(
            row,
            &mut probe,
            CreateArgumentPackageInterface::new(&arguments, named("probe"), &arguments),
        );
        let binding = create(
            row,
            &mut probe,
            CreateArgumentPackageBinding::root(named("probe"), [&arguments, &interface]),
        );
        let artifact_interface = create(
            row,
            &mut probe,
            CreateArtifactPackageInterface::new(&artifacts, named("probe"), &artifacts),
        );
        let artifact_binding = create(
            row,
            &mut probe,
            CreateArtifactPackageBinding::root(named("probe"), [&artifacts, &artifact_interface]),
        );
        ids.extend([interface, binding, artifact_interface, artifact_binding]);
    }
    ids
}

/// The four creates of slice 0066 with a generated id, after the others of
/// [`creates_of_every_kind`]: under `package_parent`, an interface of each
/// of `argument_package` and `artifact_package`, and a binding of each with
/// the second package of its family in [`check_order_start`], `q` and
/// `evidence2`.
fn interface_creates_of_every_kind(
    package_parent: &str,
    argument_package: &str,
    artifact_package: &str,
) -> Vec<Operation> {
    vec![
        CreateArgumentPackageInterface::new(package_parent, named("Interface"), argument_package)
            .into(),
        CreateArgumentPackageBinding::new(
            package_parent,
            named("Binding"),
            [argument_package, "q"],
        )
        .into(),
        CreateArtifactPackageInterface::new(
            package_parent,
            named("Artifact interface"),
            artifact_package,
        )
        .into(),
        CreateArtifactPackageBinding::new(
            package_parent,
            named("Artifact binding"),
            [artifact_package, "evidence2"],
        )
        .into(),
    ]
}

/// One check of one operation: the document `check` was given, a clone taken
/// before, and the answers of `check` and of `apply` to a clone.
struct Checked<'a> {
    context: &'a str,
    operation: &'a Operation,
    before: &'a Document,
    doc: &'a Document,
    check_answer: &'a Result<(), EditError>,
    apply_answer: &'a Result<(), EditError>,
}

/// Checks every operation of [`check_operations_for`] on every starting
/// document, alone (on the starting document itself) and in sequence (on one
/// document to which each operation is then applied), and calls `inspect` on
/// each. Returns the number of checks.
fn for_every_check(mut inspect: impl FnMut(&Checked<'_>)) -> usize {
    let mut count = 0;
    for (label, start) in starting_documents() {
        let operations = check_operations_for(&start);
        for (index, operation) in operations.iter().enumerate() {
            let before = start.clone();
            let checked = edit::check(&start, operation);
            let applied = edit::apply(&mut start.clone(), operation.clone()).map(drop);
            let context = format!("{label}, alone, operation {index} {operation:?}");
            inspect(&Checked {
                context: &context,
                operation,
                before: &before,
                doc: &start,
                check_answer: &checked,
                apply_answer: &applied,
            });
            count += 1;
        }
        let mut doc = start.clone();
        for (index, operation) in operations.iter().enumerate() {
            let before = doc.clone();
            let checked = edit::check(&doc, operation);
            let mut next = doc.clone();
            let applied = edit::apply(&mut next, operation.clone()).map(drop);
            let context = format!("{label}, in sequence, operation {index} {operation:?}");
            inspect(&Checked {
                context: &context,
                operation,
                before: &before,
                doc: &doc,
                check_answer: &checked,
                apply_answer: &applied,
            });
            doc = next;
            count += 1;
        }
    }
    count
}

/// Checks and applies `operation`, which fails two or more checks, to `doc`,
/// and asserts that both give the same answer, the first failure, which
/// `is_first` accepts and `first` names.
fn assert_first_failure(
    doc: &Document,
    operation: impl Into<Operation>,
    first: &str,
    is_first: impl Fn(&EditError) -> bool,
) {
    let operation = operation.into();
    let before = doc.clone();
    let checked = edit::check(doc, &operation);
    let applied = edit::apply(&mut doc.clone(), operation.clone()).map(drop);
    assert_eq!(
        checked, applied,
        "{EDIT_013}: {operation:?}: check and apply answer differently"
    );
    assert!(
        *doc == before,
        "{EDIT_013}: {operation:?}: the check changed the document"
    );
    assert!(
        matches!(&checked, Err(error) if is_first(error)),
        "{EDIT_013}: {operation:?} fails two checks, and the answer {checked:?} is not the first, {first}"
    );
}

#[test]
fn lib_edit_013_check_takes_the_document_and_the_operation_by_reference() {
    let check: fn(&Document, &Operation) -> Result<(), EditError> = edit::check;
    let doc = Document::new();
    let accepted = Operation::from(CreateArgumentPackage::root(named("Arguments")));
    assert_eq!(
        check(&doc, &accepted),
        Ok(()),
        "{EDIT_013}: check refused a root ArgumentPackage on an empty document"
    );
    let refused = Operation::from(CreateClaim::new(NOWHERE, named("Claim")));
    assert!(
        matches!(
            check(&doc, &refused),
            Err(EditError::ParentNotFound { ref parent, .. }) if parent.as_str() == NOWHERE
        ),
        "{EDIT_013}: check accepted a Claim under a parent the document does not hold"
    );
    // The caller still holds both operations, and the document is empty.
    assert_eq!(doc.element_count(), 0, "{EDIT_013}: check added an element");
    assert_eq!(
        edit::apply(&mut doc.clone(), accepted).map(drop),
        Ok(()),
        "{EDIT_013}: the checked operation was refused by apply"
    );
}

#[test]
fn lib_edit_013_check_answers_as_apply_for_every_operation_of_the_set() {
    let mut refusals = 0;
    let total = for_every_check(|checked| {
        assert_eq!(
            checked.check_answer, checked.apply_answer,
            "{EDIT_013}: {}: check answers otherwise than apply",
            checked.context
        );
        if checked.apply_answer.is_err() {
            refusals += 1;
        }
    });
    assert!(
        refusals > 0 && refusals < total,
        "{EDIT_013}: of {total} checks, {refusals} were refusals; the set needs both answers"
    );
}

#[test]
fn lib_edit_013_the_set_has_every_operation_struct_accepted_and_refused_with_both_id_forms() {
    // (operation name, accepted) for every operation, and (operation name,
    // caller id given, accepted) for every create.
    let mut reached: BTreeSet<(&'static str, bool)> = BTreeSet::new();
    let mut reached_creates: BTreeSet<(&'static str, bool, bool)> = BTreeSet::new();
    for_every_check(|checked| {
        let name = checked.operation.operation_name();
        let accepted = checked.apply_answer.is_ok();
        reached.insert((name, accepted));
        if let Some(caller) = has_caller_id(checked.operation) {
            reached_creates.insert((name, caller, accepted));
        }
    });
    let answer = |accepted: bool| if accepted { "accepted" } else { "refused" };
    for name in OPERATION_NAMES {
        for accepted in [true, false] {
            assert!(
                reached.contains(&(name, accepted)),
                "{EDIT_013}: the set never reaches {name} {}",
                answer(accepted)
            );
        }
    }
    for name in &OPERATION_NAMES[..CREATE_COUNT] {
        for caller in [false, true] {
            for accepted in [true, false] {
                assert!(
                    reached_creates.contains(&(*name, caller, accepted)),
                    "{EDIT_013}: the set never reaches {name} with a {} id {}",
                    if caller { "caller" } else { "generated" },
                    answer(accepted)
                );
            }
        }
    }
}

#[test]
fn lib_edit_013_check_leaves_the_document_equal_with_the_same_next_ids_of_every_kind() {
    let total = for_every_check(|checked| {
        let context = checked.context;
        assert!(
            checked.doc == checked.before,
            "{EDIT_013}: {context}: after check ({:?}) the document is not == to before",
            checked.check_answer
        );
        assert_eq!(
            next_generated_ids_of_every_kind(EDIT_013, checked.doc),
            next_generated_ids_of_every_kind(EDIT_013, checked.before),
            "{EDIT_013}: {context}: check ({:?}) changed the next generated ids",
            checked.check_answer
        );
    });
    assert!(total > 0, "{EDIT_013}: the set is empty");
}

#[test]
fn lib_edit_013_a_checked_create_of_each_kind_gets_the_generated_id_an_unchecked_one_gets() {
    let mut doc = read_text("CHECK_ORDER_START", check_order_start().as_bytes());
    let accepted = creates_of_every_kind("case", "p", "c1", "evidence");
    let refused = creates_of_every_kind(NOWHERE, NOWHERE, "c1", NOWHERE);
    assert_eq!(
        accepted.len(),
        CREATE_COUNT,
        "{EDIT_013}: setup: not one create of each kind"
    );
    for (operation, twin) in accepted.into_iter().zip(refused) {
        let mut unchecked = doc.clone();
        for _ in 0..2 {
            assert_eq!(
                edit::check(&doc, &operation),
                Ok(()),
                "{EDIT_013}: setup: check refused {operation:?}"
            );
            assert!(
                matches!(
                    edit::check(&doc, &twin),
                    Err(EditError::ParentNotFound { .. })
                ),
                "{EDIT_013}: setup: check did not refuse {twin:?} for its parent"
            );
        }
        let unchecked_id = create(EDIT_013, &mut unchecked, operation.clone());
        let checked_id = create(EDIT_013, &mut doc, operation.clone());
        assert_eq!(
            checked_id,
            unchecked_id,
            "{EDIT_013}: {}: after two checks, accepted and refused, the generated id is not the one an unchecked document gives",
            operation.operation_name()
        );
    }
}

#[test]
#[expect(
    clippy::too_many_lines,
    reason = "one case per pair of checks reads best as one list"
)]
fn lib_edit_013_a_create_failing_two_checks_gets_the_first_from_check_and_apply() {
    let doc = read_text("CHECK_ORDER_START", check_order_start().as_bytes());
    let name = || named("N");
    let none = Vec::<ElementId>::new;

    // The check order of the module docs of `edit`, "Creates".
    assert_first_failure(
        &doc,
        CreateClaim::new(NOWHERE, name()).with_id(NOT_AN_NCNAME),
        "ParentNotFound (1) before InvalidId (3)",
        |error| matches!(error, EditError::ParentNotFound { parent, .. } if parent.as_str() == NOWHERE),
    );
    assert_first_failure(
        &doc,
        CreateTaggedValue::new(NOWHERE, a_key(), two_values()).with_id("c1"),
        "ParentNotFound (1) before DuplicateId (4)",
        |error| matches!(error, EditError::ParentNotFound { parent, .. } if parent.as_str() == NOWHERE),
    );
    assert_first_failure(
        &doc,
        CreateAssertedArtifactContext::new(NOWHERE, name(), none(), NOWHERE),
        "ParentNotFound (1) before TooFewReferences (5) and ReferenceNotFound (7)",
        |error| matches!(error, EditError::ParentNotFound { parent, .. } if parent.as_str() == NOWHERE),
    );
    assert_first_failure(
        &doc,
        CreateClaim::new("case", name()).with_id("c1"),
        "InvalidParent (2) before DuplicateId (4)",
        |error| {
            matches!(
                error,
                EditError::InvalidParent { parent, parent_kind: AssuranceCasePackage, kind: Claim, .. }
                    if parent.as_str() == "case"
            )
        },
    );
    assert_first_failure(
        &doc,
        CreateArgumentPackage::new("c1", name()).with_id("c1"),
        "InvalidParent (2) before DuplicateId (4), for a package",
        |error| {
            matches!(
                error,
                EditError::InvalidParent { parent, parent_kind: Claim, kind: ArgumentPackage, .. }
                    if parent.as_str() == "c1"
            )
        },
    );
    assert_first_failure(
        &doc,
        CreateArgumentReasoning::new("case", name()).with_structure(NOWHERE),
        "InvalidParent (2) before ReferenceNotFound (7)",
        |error| {
            matches!(
                error,
                EditError::InvalidParent {
                    kind: ArgumentReasoning,
                    ..
                }
            )
        },
    );
    assert_first_failure(
        &doc,
        CreateDescription::new("d1", two_values()).with_id(NOT_AN_NCNAME),
        "InvalidParent (2: a UtilityElement contains nothing) before InvalidId (3)",
        |error| {
            matches!(
                error,
                EditError::InvalidParent {
                    parent_kind: Description,
                    kind: Description,
                    ..
                }
            )
        },
    );
    assert_first_failure(
        &doc,
        CreateImplementationConstraint::new("d1", name()).with_id("c1"),
        "InvalidParent (2) before DuplicateId (4), for a UtilityElement",
        |error| {
            matches!(
                error,
                EditError::InvalidParent {
                    kind: ImplementationConstraint,
                    ..
                }
            )
        },
    );
    assert_first_failure(
        &doc,
        CreateClaim::new("p", name()).with_id(WITH_SPACE),
        "InvalidId (3) before DuplicateId (4): `a b` is in use",
        |error| matches!(error, EditError::InvalidId { id, .. } if id.as_str() == WITH_SPACE),
    );
    assert_first_failure(
        &doc,
        CreateAssuranceCasePackage::root(name()).with_id(WITH_SPACE),
        "InvalidId (3) before DuplicateId (4), for a root",
        |error| matches!(error, EditError::InvalidId { id, .. } if id.as_str() == WITH_SPACE),
    );
    assert_first_failure(
        &doc,
        CreateNote::new("c1", name()).with_id(WITH_SPACE),
        "InvalidId (3) before DuplicateId (4), for a UtilityElement",
        |error| matches!(error, EditError::InvalidId { id, .. } if id.as_str() == WITH_SPACE),
    );
    assert_first_failure(
        &doc,
        CreateAssertedInference::new("p", name(), none(), NOWHERE).with_id("c1"),
        "DuplicateId (4) before TooFewReferences (5) and ReferenceNotFound (7)",
        |error| matches!(error, EditError::DuplicateId { id, .. } if id.as_str() == "c1"),
    );
    assert_first_failure(
        &doc,
        CreateAssertedInference::new("p", name(), none(), NOWHERE),
        "TooFewReferences (5) before ReferenceNotFound (7)",
        |error| {
            matches!(
                error,
                EditError::TooFewReferences {
                    end: "source",
                    given: 0,
                    required: 1,
                    ..
                }
            )
        },
    );
    assert_first_failure(
        &doc,
        CreateAssertedInference::new("p", name(), [NOWHERE], WITH_SPACE),
        "InvalidReference (6) of the target before ReferenceNotFound (7) of a source",
        |error| {
            matches!(
                error,
                EditError::InvalidReference { end: "target", id, .. } if id.as_str() == WITH_SPACE
            )
        },
    );
    assert_first_failure(
        &doc,
        CreateAssertedEvidence::new("p", name(), [NOWHERE, WITH_SPACE], "c1"),
        "InvalidReference (6) of the second source before ReferenceNotFound (7) of the first",
        |error| {
            matches!(
                error,
                EditError::InvalidReference { end: "source", id, .. } if id.as_str() == WITH_SPACE
            )
        },
    );
    assert_first_failure(
        &doc,
        CreateAssertedInference::new("p", name(), ["p"], NOWHERE),
        "ReferenceNotFound (7) of the target before WrongReferenceType (8) of a source",
        |error| {
            matches!(
                error,
                EditError::ReferenceNotFound { end: "target", id, .. } if id.as_str() == NOWHERE
            )
        },
    );
    assert_first_failure(
        &doc,
        CreateAssertedArtifactSupport::new("p", name(), ["p"], "c1").with_reasoning(NOWHERE),
        "ReferenceNotFound (7) of the reasoning before WrongReferenceType (8) of a source",
        |error| {
            matches!(
                error,
                EditError::ReferenceNotFound { end: "reasoning", id, .. } if id.as_str() == NOWHERE
            )
        },
    );
    assert_first_failure(
        &doc,
        CreateArtifactReference::new("p", name())
            .with_referenced_artifact_elements(["d1", NOWHERE]),
        "ReferenceNotFound (7) of the second id before WrongReferenceType (8) of the first",
        |error| {
            matches!(
                error,
                EditError::ReferenceNotFound { end: "referencedArtifactElement", id, .. }
                    if id.as_str() == NOWHERE
            )
        },
    );
    assert_first_failure(
        &doc,
        CreateArgumentGroup::new("p", name()).with_group_members(["case", WITH_SPACE]),
        "InvalidReference (6) of the second member before WrongReferenceType (8) of the first",
        |error| {
            matches!(
                error,
                EditError::InvalidReference { end: "argumentElement", id, .. }
                    if id.as_str() == WITH_SPACE
            )
        },
    );
    assert_first_failure(
        &doc,
        CreateAssertedContext::new("p", name(), ["c1"], "p").with_reasoning("c1"),
        "WrongReferenceType (8) of the target before that of the reasoning",
        |error| {
            matches!(
                error,
                EditError::WrongReferenceType {
                    end: "target",
                    kind: ArgumentPackage,
                    ..
                }
            )
        },
    );
}

#[test]
#[expect(
    clippy::too_many_lines,
    reason = "one case per pair of checks reads best as one list"
)]
fn lib_edit_013_an_edit_failing_two_checks_gets_the_first_from_check_and_apply() {
    let doc = read_text("CHECK_ORDER_START", check_order_start().as_bytes());
    let none = Vec::<ElementId>::new;

    // The check orders of the module docs of `edit`, "Operations that change
    // an element".
    assert_first_failure(
        &doc,
        SetFlag::new(NOWHERE, Flag::IsCounter, true),
        "ElementNotFound (1) before FeatureNotApplicable (2)",
        |error| matches!(error, EditError::ElementNotFound { element, .. } if element.as_str() == NOWHERE),
    );
    assert_first_failure(
        &doc,
        SetReferences::new(NOWHERE, ReferenceEnd::Source, none()),
        "ElementNotFound (1) before TooFewReferences (4)",
        |error| matches!(error, EditError::ElementNotFound { element, .. } if element.as_str() == NOWHERE),
    );
    assert_first_failure(
        &doc,
        SetReferences::new("c1", ReferenceEnd::Source, [NOWHERE]),
        "FeatureNotApplicable (2) before ReferenceNotFound (7)",
        |error| {
            matches!(
                error,
                EditError::FeatureNotApplicable { kind: Claim, feature: "source", element, .. }
                    if element.as_str() == "c1"
            )
        },
    );
    assert_first_failure(
        &doc,
        RemoveReference::new("i1", ReferenceEnd::Structure, NOWHERE),
        "FeatureNotApplicable (2) before ReferenceNotHeld (3)",
        |error| {
            matches!(
                error,
                EditError::FeatureNotApplicable {
                    kind: AssertedInference,
                    feature: "structure",
                    ..
                }
            )
        },
    );
    assert_first_failure(
        &doc,
        SetReferences::new("i1", ReferenceEnd::Target, [NOWHERE, "c1"]),
        "TooManyReferences (5) before ReferenceNotFound (7)",
        |error| {
            matches!(
                error,
                EditError::TooManyReferences {
                    end: "target",
                    given: 2,
                    allowed: 1,
                    ..
                }
            )
        },
    );
    assert_first_failure(
        &doc,
        AddReference::new("i1", ReferenceEnd::Target, NOWHERE),
        "TooManyReferences (5) before ReferenceNotFound (7), for an add",
        |error| {
            matches!(
                error,
                EditError::TooManyReferences {
                    end: "target",
                    given: 2,
                    allowed: 1,
                    ..
                }
            )
        },
    );
    assert_first_failure(
        &doc,
        SetReferences::new("i1", ReferenceEnd::Reasoning, [WITH_SPACE, NOWHERE]),
        "TooManyReferences (5) before InvalidReference (6) and ReferenceNotFound (7)",
        |error| {
            matches!(
                error,
                EditError::TooManyReferences {
                    end: "reasoning",
                    ..
                }
            )
        },
    );
    assert_first_failure(
        &doc,
        SetReferences::new("i1", ReferenceEnd::Source, [NOWHERE, WITH_SPACE]),
        "InvalidReference (6) of the second new id before ReferenceNotFound (7) of the first",
        |error| {
            matches!(
                error,
                EditError::InvalidReference { end: "source", id, .. } if id.as_str() == WITH_SPACE
            )
        },
    );
    assert_first_failure(
        &doc,
        SetReferences::new("i1", ReferenceEnd::Source, ["p", NOWHERE]),
        "ReferenceNotFound (7) of the second new id before WrongReferenceType (8) of the first",
        |error| {
            matches!(
                error,
                EditError::ReferenceNotFound { end: "source", id, .. } if id.as_str() == NOWHERE
            )
        },
    );
    assert_first_failure(
        &doc,
        ReorderElements::new(NOWHERE, none()),
        "ElementNotFound (1) before InvalidOrder (2)",
        |error| matches!(error, EditError::ElementNotFound { element, .. } if element.as_str() == NOWHERE),
    );
    assert_first_failure(
        &doc,
        ReorderElements::new("p", [NOWHERE, "c1", "c1"]),
        "ChildNotFound (3) of the first id before InvalidOrder (4) of the third",
        |error| {
            matches!(
                error,
                EditError::ChildNotFound { parent: Some(parent), child, .. }
                    if parent.as_str() == "p" && child.as_str() == NOWHERE
            )
        },
    );
    assert_first_failure(
        &doc,
        ReorderElements::new("p", ["c1", "c1", NOWHERE]),
        "InvalidOrder (4) of the second id before ChildNotFound (3) of the third",
        |error| {
            matches!(
                error,
                EditError::InvalidOrder { repeated: Some(repeated), .. } if repeated.as_str() == "c1"
            )
        },
    );
    assert_first_failure(
        &doc,
        ReorderElements::roots(["case", "case", "p"]),
        "InvalidOrder (4) of the second root before ChildNotFound (3) of a child",
        |error| {
            matches!(
                error,
                EditError::InvalidOrder { repeated: Some(repeated), .. } if repeated.as_str() == "case"
            )
        },
    );
}

// ---------------------------------------------------------------------------
// LIB-EDIT-005, LIB-EDIT-006: the deletes join the set
// ---------------------------------------------------------------------------

/// Whether `operation` is a delete.
fn is_delete(operation: &Operation) -> bool {
    matches!(operation, Operation::DeleteElement(_))
}

#[test]
fn lib_edit_005_the_delete_start_document_has_what_the_deletes_need() {
    let doc = read_text("DELETE_START", DELETE_START.as_bytes());
    let get = |value: &str| match doc.get(value) {
        Some(element) => element,
        None => panic!("{EDIT_005}: setup: the delete start document has no {value:?}"),
    };
    let roots: Vec<&str> = doc.roots().map(|root| root.id().as_str()).collect();
    assert_eq!(
        roots,
        ["dcase", "dloose"],
        "{EDIT_005}: setup: the roots of the delete start document"
    );
    assert!(
        RELATIONSHIPS.contains(&get("ctx_on_inf").kind())
            && get("ctx_on_inf")
                .target()
                .is_some_and(|target| RELATIONSHIPS.contains(&get(target.as_str()).kind())),
        "{EDIT_005}: setup: no relationship targets a relationship"
    );
    assert!(
        get("inf_on_ctx")
            .sources()
            .iter()
            .any(|source| RELATIONSHIPS.contains(&get(source.as_str()).kind())),
        "{EDIT_005}: setup: no relationship has a relationship as a source"
    );
    assert_eq!(
        (
            get("dnested").parent().map(|parent| parent.id().as_str()),
            get("strategy").structure().map(ElementId::as_str),
            get("sub_a").abstract_form().map(ElementId::as_str),
            get("deep").cited_element().map(ElementId::as_str),
        ),
        (Some("dp"), Some("dnested"), Some("goal"), Some("goal")),
        "{EDIT_005}: setup: the nested package, the structure, the abstractForm and the citation"
    );
    assert!(
        !doc.contains("ghost") && get("sub_b").meta_claims().contains(&id("ghost")),
        "{EDIT_005}: setup: the delete start document has no dangling id"
    );
}

#[test]
fn lib_edit_005_the_deletes_join_the_set_with_every_answer() {
    assert_edit_answers_and_coverage(EDIT_005, &DELETES);
}

#[test]
fn lib_edit_005_every_policy_combination_of_the_set_is_accepted_and_refused_as_its_policies_allow()
{
    let mut reached: BTreeSet<(usize, Result<(), &'static str>)> = BTreeSet::new();
    for_selected_applications(is_delete, |applied| {
        let Operation::DeleteElement(delete) = applied.operation else {
            return;
        };
        let answer = applied
            .answer
            .as_ref()
            .map(|_| ())
            .map_err(|error| error.code().as_str());
        reached.insert((policy_index(delete), answer));
    });
    for (index, delete) in deletes_of(&id("x")).iter().enumerate() {
        let mut required = vec![Ok(()), Err(Code::CMD_TARGET_NOT_FOUND.as_str())];
        if delete.reference_policy() == RejectIfReferenced {
            required.push(Err(Code::CMD_DELETE_REFERENCED.as_str()));
        }
        if delete.package_policy() == RejectIfNonEmpty {
            required.push(Err(Code::CMD_PACKAGE_NOT_EMPTY.as_str()));
        }
        if delete.cross_package_policy() == RejectIfExternalReferencesExist {
            required.push(Err(Code::CMD_EXTERNAL_REFERENCES.as_str()));
        }
        for answer in required {
            assert!(
                reached.contains(&(index, answer)),
                "{EDIT_005}: the set never reaches {delete:?} answering {answer:?}"
            );
        }
    }
}

#[test]
fn lib_edit_005_every_delete_of_the_set_deletes_and_changes_what_its_policies_give() {
    let mut accepted = 0;
    for_selected_applications(is_delete, |applied| {
        let Operation::DeleteElement(delete) = applied.operation else {
            return;
        };
        let (Ok(mutation), Ok(plan)) = (applied.answer, delete_plan(applied.before, delete)) else {
            return;
        };
        accepted += 1;
        let context = applied.context;
        assert_eq!(
            mutation.deleted_ids(),
            plan.deleted,
            "{EDIT_005}: {context}: deleted_ids() is not the deleted set of ADR 0031 in pre-order"
        );
        assert_eq!(
            mutation.changed_ids(),
            plan.changed,
            "{EDIT_005}: {context}: changed_ids() is not the surviving holders that lost ids"
        );
        assert_eq!(
            mutation.created_ids(),
            [] as [ElementId; 0],
            "{EDIT_005}: {context}: a delete created ids"
        );
    });
    assert!(accepted > 0, "{EDIT_005}: no delete of the set succeeded");
}

#[test]
fn lib_edit_005_after_every_successful_delete_of_the_set_each_surviving_end_lost_exactly_the_deleted_ids()
 {
    let mut accepted = 0;
    for_selected_applications(is_delete, |applied| {
        let Ok(mutation) = applied.answer else {
            return;
        };
        accepted += 1;
        let (context, before, after) = (applied.context, applied.before, applied.after);
        assert_structurally_valid(EDIT_005, context, after);
        assert_edit_kept_structure(EDIT_005, context, applied.operation, before, after);
        let deleted: BTreeSet<ElementId> = mutation.deleted_ids().iter().cloned().collect();
        let survivors: Vec<&ElementId> = before
            .elements()
            .map(SacmElement::id)
            .filter(|id| !deleted.contains(*id))
            .collect();
        let now: Vec<&ElementId> = after.elements().map(SacmElement::id).collect();
        assert_eq!(
            now, survivors,
            "{EDIT_005}: {context}: the survivors are not the elements before minus deleted_ids"
        );
        for element in after.elements() {
            let Some(old) = before.get(element.id()) else {
                panic!("{EDIT_005}: {context}: {:?} is new", element.id());
            };
            for rule in end_rules() {
                let expected: Vec<ElementId> = held_by(old, rule)
                    .into_iter()
                    .filter(|id| !deleted.contains(id))
                    .collect();
                assert_eq!(
                    held_by(element, rule),
                    expected,
                    "{EDIT_005}: {context}: the {} of {:?} is not its old list minus the deleted ids",
                    rule.end.name(),
                    element.id()
                );
            }
        }
        // A reference that dangled before, read from the file, still does.
        let now_bad = bad_references(after);
        for (owner, end, target) in bad_references(before) {
            if after.contains(&owner) && !before.contains(&target) {
                assert!(
                    now_bad.contains(&(owner.clone(), end, target.clone())),
                    "{EDIT_005}: {context}: the dangling {end} {target:?} of {owner:?} is gone"
                );
            }
        }
        for difference in semantic_compare(before, after) {
            let allowed = matches!(
                difference,
                Difference::LeftOnly { .. }
                    | Difference::CitedElement { .. }
                    | Difference::AbstractForm { .. }
                    | Difference::MetaClaims { .. }
                    | Difference::Reasoning { .. }
                    | Difference::Sources { .. }
                    | Difference::Target { .. }
                    | Difference::Structure { .. }
                    | Difference::ReferencedArtifactElements { .. }
                    | Difference::GroupMembers { .. }
                    | Difference::ArtifactGroupMembers { .. }
                    | Difference::ArtifactSources { .. }
                    | Difference::ArtifactTargets { .. }
            );
            // The ends of slice 0066 (LIB-EDIT-019).
            let allowed = allowed
                || matches!(
                    difference,
                    Difference::Interfaces { .. }
                        | Difference::Implements { .. }
                        | Difference::ParticipantPackages { .. }
                );
            assert!(
                allowed,
                "{EDIT_005}: {context}: the delete changed more than elements and reference ends: {difference}"
            );
        }
    });
    assert!(accepted > 0, "{EDIT_005}: no delete of the set succeeded");
}

#[test]
fn lib_edit_005_the_policies_of_the_set_cascade_scrub_and_remove_ids_from_every_end() {
    // Reference policies (by index) whose accepted deletes went beyond the
    // element's subtree; ends that lost an id on a survivor; whether a
    // relationship survived a scrub of one of its sources.
    let mut beyond: BTreeSet<usize> = BTreeSet::new();
    let mut ends: BTreeSet<&'static str> = BTreeSet::new();
    let mut scrubbed_source = false;
    for_selected_applications(is_delete, |applied| {
        let (Operation::DeleteElement(delete), Ok(mutation)) = (applied.operation, applied.answer)
        else {
            return;
        };
        let (before, after) = (applied.before, applied.after);
        let subtree = before
            .get(delete.element())
            .map(subtree_of)
            .unwrap_or_default();
        if mutation
            .deleted_ids()
            .iter()
            .any(|id| !subtree.contains(id))
        {
            beyond.insert(policy_index(delete) / 4);
        }
        for holder in mutation.changed_ids() {
            let (Some(old), Some(new)) = (before.get(holder), after.get(holder)) else {
                continue;
            };
            for rule in &END_RULES {
                if held(old, rule.end) != held(new, rule.end) {
                    ends.insert(rule.end.name());
                    if rule.end == ReferenceEnd::Source {
                        scrubbed_source = true;
                    }
                }
            }
        }
    });
    assert_eq!(
        beyond,
        BTreeSet::from([1, 2]),
        "{EDIT_005}: DeleteReferencingRelationships and ScrubReferences, and only they, delete relationships outside the subtree"
    );
    for rule in &END_RULES {
        // A survivor never loses its target: that deletes the relationship.
        if rule.end != ReferenceEnd::Target {
            assert!(
                ends.contains(rule.end.name()),
                "{EDIT_005}: no delete of the set removed an id from a {}",
                rule.end.name()
            );
        }
    }
    assert!(
        !ends.contains(ReferenceEnd::Target.name()),
        "{EDIT_005}: a relationship survived losing its target"
    );
    assert!(
        scrubbed_source,
        "{EDIT_005}: no relationship of the set survived losing one of its sources"
    );
}

#[test]
fn lib_edit_006_the_set_deletes_non_empty_elements_and_crossing_references_under_the_permissive_policies()
 {
    let mut recursive = false;
    let mut crossing = false;
    for_selected_applications(is_delete, |applied| {
        let (Operation::DeleteElement(delete), Ok(_)) = (applied.operation, applied.answer) else {
            return;
        };
        let Ok(plan) = delete_plan(applied.before, delete) else {
            return;
        };
        // A contained element that is no UtilityElement: a UtilityElement
        // contains nothing, so a direct child decides.
        let holds_a_model_element = plan.deleted.iter().any(|id| {
            applied.before.get(id).is_some_and(|element| {
                !UTILITY_ELEMENTS.contains(&element.kind())
                    && element
                        .parent()
                        .is_some_and(|parent| parent.id() == delete.element())
            })
        });
        if holds_a_model_element {
            assert!(
                delete.package_policy() == DeleteRecursively,
                "{EDIT_006}: {}: a non-empty element was deleted under RejectIfNonEmpty",
                applied.context
            );
            recursive = true;
        }
        if plan.dangling.iter().any(|(_, _, _, crosses)| *crosses) {
            assert!(
                delete.cross_package_policy() == ApplyReferencePolicy,
                "{EDIT_006}: {}: a crossing reference was handled under RejectIfExternalReferencesExist",
                applied.context
            );
            crossing = true;
        }
    });
    assert!(
        recursive,
        "{EDIT_006}: no non-empty element of the set was deleted recursively"
    );
    assert!(
        crossing,
        "{EDIT_006}: no delete of the set handled a crossing reference under ApplyReferencePolicy"
    );
}

// ---------------------------------------------------------------------------
// LIB-EDIT-004: every operation of the set can be previewed
// ---------------------------------------------------------------------------

/// Calls `inspect` with a label, the document and the operation for every
/// operation of [`check_operations_for`] on every starting document, alone
/// (on the starting document) and in sequence (on one document to which
/// each operation is then applied). Returns the number of calls.
fn for_every_state(mut inspect: impl FnMut(&str, &Document, &Operation)) -> usize {
    let mut count = 0;
    for (label, start) in starting_documents() {
        let operations = check_operations_for(&start);
        for (index, operation) in operations.iter().enumerate() {
            let context = format!("{label}, alone, operation {index} {operation:?}");
            inspect(&context, &start, operation);
            count += 1;
        }
        let mut doc = start.clone();
        for (index, operation) in operations.iter().enumerate() {
            let context = format!("{label}, in sequence, operation {index} {operation:?}");
            inspect(&context, &doc, operation);
            // Applied for the state it leaves.
            let _ = edit::apply(&mut doc, operation.clone());
            count += 1;
        }
    }
    count
}

#[test]
fn lib_edit_004_preview_answers_as_apply_and_check_for_every_operation_of_the_set() {
    let mut refusals = 0;
    let total = for_every_state(|context, doc, operation| {
        let previewed = edit::preview(doc, operation);
        let applied = edit::apply(&mut doc.clone(), operation.clone());
        assert_eq!(
            previewed
                .as_ref()
                .map(|preview| preview.mutation().clone())
                .map_err(Clone::clone),
            applied,
            "{EDIT_004}: {context}: the preview's mutation is not what apply returns"
        );
        assert_eq!(
            previewed.as_ref().map(|_| ()).map_err(Clone::clone),
            edit::check(doc, operation),
            "{EDIT_004}: {context}: preview answers otherwise than check"
        );
        match &previewed {
            Ok(preview) => {
                assert_eq!(
                    preview.operation(),
                    operation,
                    "{EDIT_004}: {context}: operation() is not the operation previewed"
                );
                assert!(
                    preview.revision() == doc.revision(),
                    "{EDIT_004}: {context}: revision() is not the document's"
                );
            }
            Err(_) => refusals += 1,
        }
    });
    assert!(
        refusals > 0 && refusals < total,
        "{EDIT_004}: of {total} previews, {refusals} were refusals; the set needs both answers"
    );
}

#[test]
fn lib_edit_004_preview_leaves_every_document_of_the_set_equal_with_the_same_revision_and_next_ids()
{
    let total = for_every_state(|context, doc, operation| {
        let before = doc.clone();
        let revision = doc.revision();
        let answer = edit::preview(doc, operation).map(|_| ());
        assert!(
            *doc == before,
            "{EDIT_004}: {context}: after preview ({answer:?}) the document is not == to before"
        );
        assert!(
            doc.revision() == revision,
            "{EDIT_004}: {context}: preview ({answer:?}) changed the revision"
        );
        assert_eq!(
            next_generated_ids_of_every_kind(EDIT_004, doc),
            next_generated_ids_of_every_kind(EDIT_004, &before),
            "{EDIT_004}: {context}: preview ({answer:?}) changed the next generated ids"
        );
    });
    assert!(total > 0, "{EDIT_004}: the set is empty");
}

#[test]
fn lib_edit_004_the_dangling_references_of_every_preview_of_the_set_are_those_adr_0031_defines() {
    let mut crossing = (false, false);
    for_every_state(|context, doc, operation| {
        let Ok(preview) = edit::preview(doc, operation) else {
            return;
        };
        let listed: Vec<Dangling> = preview
            .dangling_references()
            .iter()
            .map(|dangling| {
                (
                    dangling.holder().clone(),
                    dangling.end(),
                    dangling.id().clone(),
                    dangling.crosses_package(),
                )
            })
            .collect();
        let expected = match operation {
            Operation::DeleteElement(delete) => match delete_plan(doc, delete) {
                Ok(plan) => plan.dangling,
                Err(code) => {
                    panic!("{EDIT_004}: {context}: previewed, and the oracle refuses {code:?}")
                }
            },
            _ => Vec::new(),
        };
        assert_eq!(
            listed, expected,
            "{EDIT_004}: {context}: dangling_references() is not the set and order of ADR 0031"
        );
        for (_, _, _, crosses) in &listed {
            if *crosses {
                crossing.0 = true;
            } else {
                crossing.1 = true;
            }
        }
    });
    assert_eq!(
        crossing,
        (true, true),
        "{EDIT_004}: the set lists no crossing or no non-crossing dangling reference"
    );
}

// ---------------------------------------------------------------------------
// LIB-EDIT-017, LIB-EDIT-018: moves and copies join the set
// ---------------------------------------------------------------------------

/// Whether `operation` is a move.
fn is_move(operation: &Operation) -> bool {
    matches!(operation, Operation::MoveElement(_))
}

/// Whether `operation` is a copy.
fn is_copy(operation: &Operation) -> bool {
    matches!(operation, Operation::CopyElements(_))
}

#[test]
fn lib_edit_017_move_joins_the_set_with_every_answer() {
    // Every answer of the oracle (written from ADR 0036 and this file's
    // containment table), and an Ok with and without changed ids. The other
    // tests of the set check that the moves keep the document structurally
    // valid, leave a refused document unchanged, and give the Mutation that
    // semantic_compare reports, from check, preview and apply alike.
    assert_edit_answers_and_coverage(EDIT_017, &MOVES);

    let mut between = 0;
    for_selected_applications(is_move, |applied| {
        let (Operation::MoveElement(moved), Ok(mutation)) = (applied.operation, applied.answer)
        else {
            return;
        };
        let (context, before, after) = (applied.context, applied.before, applied.after);
        let element = moved.element();
        let (Some(old), Some(new)) = (before.get(element), after.get(element)) else {
            panic!("{EDIT_017}: {context}: the moved element is gone");
        };
        assert_eq!(
            new.parent().map(SacmElement::id),
            moved.parent(),
            "{EDIT_017}: {context}: the element is not under the parent given"
        );
        let siblings: Vec<&ElementId> = match new.parent() {
            Some(parent) => parent.children().map(SacmElement::id).collect(),
            None => after.roots().map(SacmElement::id).collect(),
        };
        let others = siblings.len().saturating_sub(1);
        assert_eq!(
            siblings.iter().position(|sibling| *sibling == element),
            Some(moved.position().unwrap_or(others)),
            "{EDIT_017}: {context}: the element is not at the position given, or last"
        );
        assert_eq!(
            subtree_of(new),
            subtree_of(old),
            "{EDIT_017}: {context}: the subtree changed"
        );
        assert_eq!(
            after.element_count(),
            before.element_count(),
            "{EDIT_017}: {context}: the element count changed"
        );
        for holder in before.elements() {
            let Some(now) = after.get(holder.id()) else {
                panic!("{EDIT_017}: {context}: {:?} is gone", holder.id());
            };
            for rule in end_rules() {
                assert_eq!(
                    held_by(now, rule),
                    held_by(holder, rule),
                    "{EDIT_017}: {context}: the {} of {:?} changed",
                    rule.end.name(),
                    holder.id()
                );
            }
        }
        assert_eq!(
            next_generated_ids(EDIT_017, after),
            next_generated_ids(EDIT_017, before),
            "{EDIT_017}: {context}: the move changed the next generated ids"
        );
        for difference in semantic_compare(before, after) {
            assert!(
                matches!(
                    difference,
                    Difference::Parent { .. }
                        | Difference::ChildOrder { .. }
                        | Difference::RootOrder { .. }
                ),
                "{EDIT_017}: {context}: the move changed more than a parent and an order: {difference}"
            );
        }
        if old.parent().map(SacmElement::id) != new.parent().map(SacmElement::id) {
            between += 1;
            assert!(
                mutation.changed_ids().contains(element),
                "{EDIT_017}: {context}: a move to another parent does not change the element"
            );
        }
    });
    assert!(
        between > 0,
        "{EDIT_017}: no move of the set changed an element's parent"
    );
}

#[test]
fn lib_edit_018_copy_joins_the_set_with_every_answer() {
    // As for the moves, the other tests of the set check what every
    // operation must keep; here, what a copy must make.
    assert_edit_answers_and_coverage(EDIT_018, &COPIES);

    // (from the document itself, from the second source); kept and
    // generated ids.
    let mut accepted = (0, 0);
    let (mut kept, mut generated) = (0, 0);
    for_selected_applications(is_copy, |applied| {
        let (Operation::CopyElements(copy), Ok(mutation)) = (applied.operation, applied.answer)
        else {
            return;
        };
        let context = applied.context;
        let (more_kept, more_generated) =
            assert_copy_made(context, applied.before, applied.after, copy, mutation);
        kept += more_kept;
        generated += more_generated;
        if source_of(copy).1 {
            accepted.0 += 1;
        } else {
            accepted.1 += 1;
        }
        assert!(
            mutation.changed_ids().is_empty() && mutation.deleted_ids().is_empty(),
            "{EDIT_018}: {context}: a copy changed or deleted ids: {mutation:?}"
        );
        assert_eq!(
            applied.after.element_count(),
            applied.before.element_count() + copy.source_ids().len(),
            "{EDIT_018}: {context}: the element count"
        );
    });
    assert!(
        accepted.0 > 0 && accepted.1 > 0,
        "{EDIT_018}: the set accepts no copy from the document itself or from the second \
         source: {accepted:?}"
    );
    assert!(
        kept > 0 && generated > 0,
        "{EDIT_018}: the set's copies never keep ({kept}) or never generate ({generated}) an id"
    );
}

// ---------------------------------------------------------------------------
// LIB-API-014: the referrers after every operation of the set
// ---------------------------------------------------------------------------

/// `doc.referrers(id)` equals a full scan through `SacmElement::references`
/// (holder in pre-order, end in `ReferenceEnd::ALL` order, which
/// [`END_RULES`] follows, then position) for every id any end holds, and
/// is empty for every element id no end holds.
fn assert_referrers_are_a_full_scan(context: &str, doc: &Document) {
    let mut scan: BTreeMap<ElementId, Vec<(ElementId, &'static str, usize)>> = BTreeMap::new();
    for holder in doc.elements() {
        for end in ends_in_order() {
            for (position, value) in holder.references(end).iter().enumerate() {
                scan.entry(value.clone()).or_default().push((
                    holder.id().clone(),
                    end.name(),
                    position,
                ));
            }
        }
    }
    for (value, expected) in &scan {
        let listed: Vec<(ElementId, &'static str, usize)> = doc
            .referrers(value)
            .iter()
            .map(|referrer| {
                (
                    referrer.holder().id().clone(),
                    referrer.end().name(),
                    referrer.position(),
                )
            })
            .collect();
        assert_eq!(
            &listed, expected,
            "{API_014}: {context}: the referrers of {value:?} are not the full scan"
        );
    }
    for element in doc.elements() {
        if !scan.contains_key(element.id()) {
            assert!(
                doc.referrers(element.id()).is_empty(),
                "{API_014}: {context}: {:?}, which no end holds, has referrers",
                element.id()
            );
        }
    }
}

#[test]
fn lib_api_014_referrers_equal_a_full_scan_after_every_operation_of_the_set() {
    for (label, start) in starting_documents() {
        assert_referrers_are_a_full_scan(&format!("{label}, as read"), &start);
    }
    let mut reached: BTreeSet<&'static str> = BTreeSet::new();
    let total = for_every_application(|applied| {
        if applied.answer.is_err() {
            return;
        }
        assert_referrers_are_a_full_scan(applied.context, applied.after);
        reached.insert(match classify(applied.operation) {
            Class::Create { .. } => "create",
            Class::Edit { label } => label,
        });
    });
    assert!(total > 0, "{API_014}: the set is empty");
    for label in [
        "create",
        "SetReferences",
        "AddReference",
        "RemoveReference",
        "ReorderElements",
    ]
    .into_iter()
    .chain(MOVES)
    .chain(COPIES)
    .chain(DELETES)
    {
        assert!(
            reached.contains(label),
            "{API_014}: no {label} of the set succeeded"
        );
    }
}
