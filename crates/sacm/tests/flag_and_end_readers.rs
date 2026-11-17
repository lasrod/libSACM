//! Tests for the public flag and reference-end readers (LIB-API-015; ADR
//! 0029 amendment 1; formal/23-05-08 §11.13 (p. 40) "target:ArgumentAsset[1]",
//! §12.14 (p. 50) "target:ArtifactAsset[1..*]"; §10.3, §10.8 to §10.11 for
//! the Terminology ends of slice 0062).
//!
//! - `SacmElement::flag(Flag)` and `SacmElement::references(ReferenceEnd)`:
//!   a flag the kind lacks is `false`, an end it lacks holds no id, and
//!   otherwise they equal the named accessors.
//! - `Flag::ALL` and `ReferenceEnd::ALL` are `&'static` slices in a
//!   documented order: the flags as `compare` reports them, the ends in
//!   write order.
//! - `Flag::applies_to(kind)` and `ReferenceEnd::applies_to(kind)` agree
//!   with the edits: true exactly when a `SetFlag`, or a
//!   `SetReferences::clear`, is not `FeatureNotApplicable`.
//! - `ReferenceEnd::bounds(kind)` gives `(lower, upper)`, `None` for no
//!   upper bound, and `None` when the kind has no such end; it agrees with
//!   `SetReferences`' `TooFewReferences` and `TooManyReferences`.
//!
//! This file's own owner table, written from the standard, is a second
//! oracle, independent of the crate's tables.

use sacm::edit::{self, EditError, Operation, SetFlag, SetReferences};
use sacm::model::ElementKind::{
    ArgumentGroup, ArgumentReasoning, ArtifactAssetRelationship, ArtifactGroup, ArtifactPackage,
    ArtifactReference, AssertedArtifactContext, AssertedArtifactSupport, AssertedContext,
    AssertedEvidence, AssertedInference, Category, Claim, Expression, Term, TerminologyGroup,
};
use sacm::model::{Document, ElementId, ElementKind, Flag, ReferenceEnd, SacmElement};
use sacm::xmi::{self, Mode, ReadOptions, WriteOptions};

const ROW: &str = "LIB-API-015";

/// The five asserted relationships (§11.13-§11.18).
const RELATIONSHIPS: [ElementKind; 5] = [
    AssertedInference,
    AssertedEvidence,
    AssertedContext,
    AssertedArtifactSupport,
    AssertedArtifactContext,
];

/// The Assertions (§11.10): a Claim and the asserted relationships.
const ASSERTIONS: [ElementKind; 6] = [
    Claim,
    AssertedInference,
    AssertedEvidence,
    AssertedContext,
    AssertedArtifactSupport,
    AssertedArtifactContext,
];

/// Whether an element of `kind` has `end`, by the classes that own the ends
/// (§8.2, §10.3, §10.8 to §10.11, §11.2, §11.9, §11.10, §11.12, §11.13,
/// §12.3, §12.14).
fn owns(end: ReferenceEnd, kind: ElementKind) -> bool {
    match end {
        ReferenceEnd::CitedElement | ReferenceEnd::AbstractForm => true,
        ReferenceEnd::MetaClaim => ASSERTIONS.contains(&kind),
        ReferenceEnd::Reasoning | ReferenceEnd::Source | ReferenceEnd::Target => {
            RELATIONSHIPS.contains(&kind)
        }
        ReferenceEnd::Structure => kind == ArgumentReasoning,
        ReferenceEnd::ReferencedArtifactElement => kind == ArtifactReference,
        ReferenceEnd::GroupMember => kind == ArgumentGroup,
        ReferenceEnd::ArtifactGroupMember => kind == ArtifactGroup,
        ReferenceEnd::ArtifactSource | ReferenceEnd::ArtifactTarget => {
            kind == ArtifactAssetRelationship
        }
        // Every TerminologyAsset: a Category and both ExpressionElements.
        ReferenceEnd::Category => [Category, Term, Expression].contains(&kind),
        ReferenceEnd::Element => kind == Expression,
        ReferenceEnd::Origin => kind == Term,
        ReferenceEnd::TerminologyGroupMember => kind == TerminologyGroup,
        other => panic!("{ROW}: an end this test does not know: {other:?}"),
    }
}

/// Whether an element of `kind` has `flag` (§8.2, §11.13).
fn has_flag(flag: Flag, kind: ElementKind) -> bool {
    match flag {
        Flag::IsCitation | Flag::IsAbstract => true,
        Flag::IsCounter => RELATIONSHIPS.contains(&kind),
        other => panic!("{ROW}: a flag this test does not know: {other:?}"),
    }
}

/// The bounds of `end` on its owners, from the standard: `source` `[1..*]`
/// and `target` `[1]` (§11.13), an `ArtifactAssetRelationship`'s `source` and
/// `target` `[1..*]` (§12.14), the many-valued ends `[0..*]` (with
/// `category`, an Expression's `element` and a `TerminologyGroup`'s
/// `terminologyElement`, §10.3, §10.8 to §10.10), the others `[0..1]` (with
/// a Term's `origin`, §10.11).
fn standard_bounds(end: ReferenceEnd) -> (usize, Option<usize>) {
    match end {
        ReferenceEnd::Source | ReferenceEnd::ArtifactSource | ReferenceEnd::ArtifactTarget => {
            (1, None)
        }
        ReferenceEnd::Target => (1, Some(1)),
        ReferenceEnd::MetaClaim
        | ReferenceEnd::ReferencedArtifactElement
        | ReferenceEnd::GroupMember
        | ReferenceEnd::ArtifactGroupMember
        | ReferenceEnd::Category
        | ReferenceEnd::Element
        | ReferenceEnd::TerminologyGroupMember => (0, None),
        _ => (0, Some(1)),
    }
}

/// A document with one element of every kind of `ElementKind::ALL`, each
/// with the id `e_<n>`, `n` its index in `ALL`: a package as a root, and
/// every other kind in an ArtifactPackage, which holds every `ModelElement`
/// in `artifactElement` (§12.2, ADR 0014) and every `UtilityElement` in the
/// roles of a `ModelElement` (§8.6). Returns the document and the id of
/// each kind's element.
fn every_kind() -> (Document, Vec<(ElementKind, ElementId)>) {
    let mut doc = Document::new();
    let holder = ElementId::from("holder");
    let insert =
        |doc: &mut Document, parent: Option<&ElementId>, kind: ElementKind, value: &ElementId| {
            if let Err(error) = doc.insert(parent, kind, value.clone(), None) {
                panic!("{ROW}: setup: cannot insert the {kind} {value:?}: {error}");
            }
        };
    insert(&mut doc, None, ArtifactPackage, &holder);
    let mut elements = Vec::new();
    for (index, &kind) in ElementKind::ALL.iter().enumerate() {
        let value = ElementId::from(format!("e_{index}"));
        let parent = if kind.is_package() {
            None
        } else {
            Some(&holder)
        };
        insert(&mut doc, parent, kind, &value);
        elements.push((kind, value));
    }
    (doc, elements)
}

/// The answer of `operation` on a clone of `doc`.
fn answer(doc: &Document, operation: impl Into<Operation>) -> Result<(), EditError> {
    edit::apply(&mut doc.clone(), operation).map(drop)
}

/// The ids `element` holds in `end`, through the named accessors.
fn named_accessor(element: SacmElement<'_>, end: ReferenceEnd) -> Vec<ElementId> {
    fn one(held: Option<&ElementId>) -> Vec<ElementId> {
        held.into_iter().cloned().collect()
    }
    match end {
        ReferenceEnd::CitedElement => one(element.cited_element()),
        ReferenceEnd::AbstractForm => one(element.abstract_form()),
        ReferenceEnd::MetaClaim => element.meta_claims().to_vec(),
        ReferenceEnd::Reasoning => one(element.reasoning()),
        // `sources()` and `targets()` answer for both relationship kinds
        // (ADR 0034 amendment 2): each end is the sources or targets of its
        // own kind only; `target()` is an asserted relationship's.
        ReferenceEnd::Source if element.kind() != ArtifactAssetRelationship => {
            element.sources().to_vec()
        }
        ReferenceEnd::ArtifactSource if element.kind() == ArtifactAssetRelationship => {
            element.sources().to_vec()
        }
        ReferenceEnd::ArtifactTarget if element.kind() == ArtifactAssetRelationship => {
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
        ReferenceEnd::GroupMember if element.kind() == ArgumentGroup => {
            element.group_members().to_vec()
        }
        ReferenceEnd::ArtifactGroupMember if element.kind() == ArtifactGroup => {
            element.group_members().to_vec()
        }
        ReferenceEnd::TerminologyGroupMember if element.kind() == TerminologyGroup => {
            element.group_members().to_vec()
        }
        ReferenceEnd::GroupMember
        | ReferenceEnd::ArtifactGroupMember
        | ReferenceEnd::TerminologyGroupMember => Vec::new(),
        ReferenceEnd::Category => element.categories().to_vec(),
        ReferenceEnd::Element => element.expression_elements().to_vec(),
        ReferenceEnd::Origin => one(element.origin()),
        other => panic!("{ROW}: an end this test does not know: {other:?}"),
    }
}

/// A document in which every end and flag is set somewhere: the edit start
/// document of `edit_atomic.rs`, with an ArtifactPackage holding an
/// `ArtifactGroup` with members and an `ArtifactAssetRelationship` with
/// sources and targets, a counter relationship, and a TerminologyPackage
/// whose Category, Term, Expression and `TerminologyGroup` hold the
/// Terminology ends (slice 0062).
const READERS_START: &str = r#"<xmi:XMI xmlns:sacm="http://www.omg.org/spec/SACM/20220301" xmlns:xmi="http://www.omg.org/spec/XMI/20131001">
  <sacm:AssuranceCasePackage xmi:id="case">
    <name content="Case"/>
    <argumentPackage xmi:id="p">
      <name content="P"/>
      <argumentElement xmi:type="sacm:Claim" xmi:id="c1" isCitation="true" citedElement="c2" isAbstract="true" abstractForm="m1" metaClaim="m1 x m1"><name content="C1"/></argumentElement>
      <argumentElement xmi:type="sacm:Claim" xmi:id="c2"><name content="C2"/></argumentElement>
      <argumentElement xmi:type="sacm:Claim" xmi:id="m1"><name content="M1"/></argumentElement>
      <argumentElement xmi:type="sacm:ArgumentReasoning" xmi:id="why" structure="p"><name content="Why"/></argumentElement>
      <argumentElement xmi:type="sacm:ArtifactReference" xmi:id="r1" referencedArtifactElement="c1 t1"><name content="R1"/></argumentElement>
      <argumentElement xmi:type="sacm:ArgumentGroup" xmi:id="g1" argumentElement="c1 c2"><name content="G1"/></argumentElement>
      <argumentElement xmi:type="sacm:AssertedInference" xmi:id="i1" isCounter="true" source="c2 m1" target="c1" reasoning="why" metaClaim="m1"><name content="I1"/></argumentElement>
    </argumentPackage>
    <artifactPackage xmi:id="arts">
      <name content="Arts"/>
      <artifactElement xmi:type="sacm:Technique" xmi:id="t1"><name content="T1"/></artifactElement>
      <artifactElement xmi:type="sacm:ArtifactGroup" xmi:id="ag1" artifactElement="t1 c1"><name content="Ag1"/></artifactElement>
      <artifactElement xmi:type="sacm:Activity" xmi:id="act1"><name content="Act1"/></artifactElement>
      <artifactElement xmi:type="sacm:ArtifactAssetRelationship" xmi:id="rel1" source="t1" target="act1 t1"><name content="Rel1"/></artifactElement>
    </artifactPackage>
    <terminologyPackage xmi:id="terms">
      <name content="Terms"/>
      <terminologyElement xmi:type="sacm:Category" xmi:id="k1" category="k1"><name content="K1"/></terminologyElement>
      <terminologyElement xmi:type="sacm:Term" xmi:id="x1" value="x" origin="c1" category="k1"><name content="X1"/></terminologyElement>
      <terminologyElement xmi:type="sacm:Expression" xmi:id="e1" value="e" element="x1 x1" category="k1"><name content="E1"/></terminologyElement>
      <terminologyElement xmi:type="sacm:TerminologyGroup" xmi:id="tg1" terminologyElement="x1 e1"><name content="TG1"/></terminologyElement>
    </terminologyPackage>
  </sacm:AssuranceCasePackage>
</xmi:XMI>
"#;

// ---------------------------------------------------------------------------
// Acceptance 1: applies_to agrees with the edits
// ---------------------------------------------------------------------------

#[test]
fn lib_api_015_applies_to_agrees_with_set_flag_and_set_references_for_every_kind() {
    let (doc, elements) = every_kind();
    assert_eq!(
        elements.len(),
        ElementKind::ALL.len(),
        "{ROW}: setup: one element of every kind"
    );
    for (kind, element) in &elements {
        let kind = *kind;
        for &flag in Flag::ALL {
            let refused = matches!(
                answer(&doc, SetFlag::new(element, flag, true)),
                Err(EditError::FeatureNotApplicable { .. })
            );
            assert_eq!(
                flag.applies_to(kind),
                !refused,
                "{ROW}: Flag::{flag:?}.applies_to({kind}) disagrees with SetFlag"
            );
            assert_eq!(
                flag.applies_to(kind),
                has_flag(flag, kind),
                "{ROW}: Flag::{flag:?}.applies_to({kind}) disagrees with the standard"
            );
        }
        for &end in ReferenceEnd::ALL {
            let refused = matches!(
                answer(&doc, SetReferences::clear(element, end)),
                Err(EditError::FeatureNotApplicable { .. })
            );
            assert_eq!(
                end.applies_to(kind),
                !refused,
                "{ROW}: ReferenceEnd::{end:?}.applies_to({kind}) disagrees with SetReferences::clear"
            );
            assert_eq!(
                end.applies_to(kind),
                owns(end, kind),
                "{ROW}: ReferenceEnd::{end:?}.applies_to({kind}) disagrees with the standard"
            );
        }
    }
}

// ---------------------------------------------------------------------------
// Acceptance 2: bounds
// ---------------------------------------------------------------------------

#[test]
fn lib_api_015_bounds_agree_with_the_reference_edit_refusals_for_every_kind() {
    // The values the row names.
    assert_eq!(
        ReferenceEnd::Source.bounds(AssertedInference),
        Some((1, None)),
        "{ROW}: source on an AssertedInference is [1..*]"
    );
    assert_eq!(
        ReferenceEnd::Target.bounds(AssertedInference),
        Some((1, Some(1))),
        "{ROW}: target on an AssertedInference is [1] (§11.13)"
    );
    assert_eq!(
        ReferenceEnd::CitedElement.bounds(Claim),
        Some((0, Some(1))),
        "{ROW}: citedElement on a Claim is [0..1]"
    );
    assert_eq!(
        ReferenceEnd::MetaClaim.bounds(Claim),
        Some((0, None)),
        "{ROW}: metaClaim on a Claim is [0..*]"
    );
    assert_eq!(
        ReferenceEnd::Source.bounds(Claim),
        None,
        "{ROW}: a Claim has no source"
    );

    let (doc, elements) = every_kind();
    for (kind, element) in &elements {
        let kind = *kind;
        for &end in ReferenceEnd::ALL {
            let bounds = end.bounds(kind);
            assert_eq!(
                bounds.is_some(),
                end.applies_to(kind),
                "{ROW}: {end:?}.bounds({kind}) is {bounds:?}, and applies_to says otherwise"
            );
            let Some((lower, upper)) = bounds else {
                continue;
            };
            assert_eq!(
                (lower, upper),
                standard_bounds(end),
                "{ROW}: {end:?}.bounds({kind}) disagrees with the standard"
            );
            // Ids that name nothing: the bounds are checked first (checks 4
            // and 5), so any other answer is no multiplicity refusal.
            for given in 0..=3 {
                let list = vec![ElementId::from("nowhere"); given];
                let refusal = answer(&doc, SetReferences::new(element, end, list));
                let too_few = matches!(
                    &refusal,
                    Err(EditError::TooFewReferences { given: g, required, .. }) if *g == given && *required == lower
                );
                let too_many = matches!(
                    &refusal,
                    Err(EditError::TooManyReferences { given: g, allowed, .. })
                        if *g == given && Some(*allowed) == upper
                );
                let multiplicity = matches!(
                    &refusal,
                    Err(EditError::TooFewReferences { .. } | EditError::TooManyReferences { .. })
                );
                if given < lower {
                    assert!(
                        too_few,
                        "{ROW}: {end:?} on a {kind} with {given} ids, below the bounds {bounds:?}, is {refusal:?}"
                    );
                } else if upper.is_some_and(|upper| given > upper) {
                    assert!(
                        too_many,
                        "{ROW}: {end:?} on a {kind} with {given} ids, above the bounds {bounds:?}, is {refusal:?}"
                    );
                } else {
                    assert!(
                        !multiplicity,
                        "{ROW}: {end:?} on a {kind} with {given} ids, within the bounds {bounds:?}, is {refusal:?}"
                    );
                }
            }
        }
    }
}

// ---------------------------------------------------------------------------
// Acceptance 3: the readers equal the named accessors
// ---------------------------------------------------------------------------

#[test]
fn lib_api_015_references_and_flag_equal_the_named_accessors() {
    let doc = match xmi::read(READERS_START.as_bytes(), &ReadOptions::default()) {
        Ok(output) => output.into_document(),
        Err(error) => panic!("{ROW}: setup: READERS_START does not read: {error}"),
    };
    let mut held_somewhere = Vec::new();
    for element in doc.elements() {
        let kind = element.kind();
        for &end in ReferenceEnd::ALL {
            let read = element.references(end);
            assert_eq!(
                read,
                named_accessor(element, end).as_slice(),
                "{ROW}: references({end:?}) of {:?} is not its named accessor",
                element.id()
            );
            if !end.applies_to(kind) {
                assert!(
                    read.is_empty(),
                    "{ROW}: the {kind} {:?} holds ids in {end:?}, which it lacks",
                    element.id()
                );
            }
            if !read.is_empty() {
                held_somewhere.push(end);
            }
        }
        let flags = [
            (Flag::IsCitation, element.is_citation()),
            (Flag::IsAbstract, element.is_abstract()),
            (Flag::IsCounter, element.is_counter()),
        ];
        for (flag, value) in flags {
            assert_eq!(
                element.flag(flag),
                value,
                "{ROW}: flag({flag:?}) of {:?} is not its named accessor",
                element.id()
            );
            if !flag.applies_to(kind) {
                assert!(!element.flag(flag), "{ROW}: a flag {kind} lacks is true");
            }
        }
    }
    for &end in ReferenceEnd::ALL {
        assert!(
            held_somewhere.contains(&end),
            "{ROW}: setup: no element of READERS_START holds a {end:?}"
        );
    }
    // An ArtifactGroup's members are its artifactElement, not its
    // argumentElement; an ArgumentGroup's the other way round.
    let get = |value: &str| match doc.get(value) {
        Some(found) => found,
        None => panic!("{ROW}: setup: no {value:?}"),
    };
    let artifact_group = get("ag1");
    assert!(
        artifact_group
            .references(ReferenceEnd::GroupMember)
            .is_empty(),
        "{ROW}: GroupMember on an ArtifactGroup holds ids"
    );
    assert_eq!(
        artifact_group.references(ReferenceEnd::ArtifactGroupMember),
        [ElementId::from("t1"), ElementId::from("c1")],
        "{ROW}: ArtifactGroupMember on an ArtifactGroup"
    );
    assert!(
        get("g1")
            .references(ReferenceEnd::ArtifactGroupMember)
            .is_empty(),
        "{ROW}: ArtifactGroupMember on an ArgumentGroup holds ids"
    );
    // A repeated id is held at each position, a dangling one as read.
    assert_eq!(
        get("c1").references(ReferenceEnd::MetaClaim),
        [
            ElementId::from("m1"),
            ElementId::from("x"),
            ElementId::from("m1")
        ],
        "{ROW}: metaClaim of c1"
    );
    assert!(
        get("i1").flag(Flag::IsCounter) && !get("c1").flag(Flag::IsCounter),
        "{ROW}: isCounter of i1 and of the Claim c1"
    );
    assert_relationship_ends(&doc);
}

/// An `ArtifactAssetRelationship`'s ends are its own, not an asserted
/// relationship's, and the other way round (ADR 0034 amendment 2): in
/// [`READERS_START`], `rel1` and `i1`.
fn assert_relationship_ends(doc: &Document) {
    let get = |value: &str| match doc.get(value) {
        Some(found) => found,
        None => panic!("{ROW}: setup: no {value:?}"),
    };
    let link = get("rel1");
    assert_eq!(
        (
            link.references(ReferenceEnd::ArtifactSource),
            link.references(ReferenceEnd::ArtifactTarget),
            link.references(ReferenceEnd::Source),
            link.references(ReferenceEnd::Target),
        ),
        (
            [ElementId::from("t1")].as_slice(),
            [ElementId::from("act1"), ElementId::from("t1")].as_slice(),
            [].as_slice(),
            [].as_slice(),
        ),
        "{ROW}: the ends of rel1"
    );
    assert!(
        get("i1")
            .references(ReferenceEnd::ArtifactSource)
            .is_empty()
            && get("i1")
                .references(ReferenceEnd::ArtifactTarget)
                .is_empty(),
        "{ROW}: ArtifactSource or ArtifactTarget on an AssertedInference holds ids"
    );
}

// ---------------------------------------------------------------------------
// Acceptance 4: ALL
// ---------------------------------------------------------------------------

#[test]
fn lib_api_015_all_lists_every_flag_and_end_in_order() {
    let flags: &'static [Flag] = Flag::ALL;
    assert_eq!(
        flags,
        [Flag::IsCitation, Flag::IsAbstract, Flag::IsCounter],
        "{ROW}: Flag::ALL"
    );
    let ends: &'static [ReferenceEnd] = ReferenceEnd::ALL;
    assert_eq!(
        ends,
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
    // Write order: an element's ends are written in ALL order. i1 of
    // READERS_START is read with its ends in another order.
    let doc = match xmi::read(READERS_START.as_bytes(), &ReadOptions::default()) {
        Ok(output) => output.into_document(),
        Err(error) => panic!("{ROW}: setup: READERS_START does not read: {error}"),
    };
    // Tolerant: c1's metaClaim x dangles, which a strict write refuses.
    let text = match xmi::write(&doc, &WriteOptions::default().with_mode(Mode::Tolerant)) {
        Ok(text) => text,
        Err(error) => panic!("{ROW}: setup: the write was refused: {error}"),
    };
    // The ends of the holder's kind: no kind has two ends of one name (ADR
    // 0034 amendment 2), so each is found by its name.
    for (holder, least) in [("c1", 3), ("i1", 3), ("rel1", 2), ("x1", 2), ("e1", 2)] {
        let Some(line) = text
            .lines()
            .find(|line| line.contains(&format!("xmi:id=\"{holder}\"")))
        else {
            panic!("{ROW}: {holder:?} is not written:\n{text}");
        };
        let Some(kind) = doc.get(holder).map(SacmElement::kind) else {
            panic!("{ROW}: setup: no {holder:?}");
        };
        let written: Vec<usize> = ReferenceEnd::ALL
            .iter()
            .filter(|end| end.applies_to(kind))
            .filter_map(|end| line.find(&format!(" {}=\"", end.name())))
            .collect();
        assert!(
            written.len() >= least && written.windows(2).all(|pair| pair[0] < pair[1]),
            "{ROW}: the ends of {holder:?} are not written in ReferenceEnd::ALL order: {line}"
        );
    }
}
