//! Tests for containment legality (LIB-API-008, D-VAL-LAYER, ADR 0014).
//!
//! Every `Document::insert` under a parent refuses a child kind that no SACM
//! 2.3 composite role of the parent's class admits, with
//! `InsertError::ContainmentNotAllowed`, and leaves the document unchanged.
//! Roots are not restricted, and multiplicity is not enforced.
//!
//! The role table is crate-private, so everything here goes through `insert`.
//! Item 1 spells the table out by hand; item 9 derives it from
//! `docs/sacm/metamodel-inventory.md`. Two independent sources must agree with
//! the implementation.
//!
//! Documents are built through `Document::insert`, the `test-support` seam. The
//! crate dev-depends on itself with that feature, so these tests always see it.
//!
//! Not checked here: that the crate-internal insertion used by the XMI reader
//! and edit operations applies the same rule. It has no public entry point
//! yet; SACM23-XMI-016 and the edit rows test it when they arrive.

use std::collections::BTreeSet;
use std::path::Path;

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
    AbstractClass, Document, ElementId, ElementKind, InsertError, LangString, SacmElement,
};

// --- The containment table, spelled out ----------------------------------

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

/// ArtifactPackage, its interface and its binding (§12.2-§12.4).
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
/// (§12.6; `ArtifactAssetRelationship` by §12.14 "Superclass `ArtifactAsset`",
/// D-TEXT-MODEL).
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

/// The number of legal (parent kind, child kind) pairs.
const LEGAL_PAIRS: usize = 339;

/// Every legal (parent kind, child kind) pair, from the 12 element-typed
/// composite roles, all `0..*`.
fn containment_table() -> BTreeSet<(ElementKind, ElementKind)> {
    let mut table = BTreeSet::new();
    let mut admit = |parents: &[ElementKind], children: &[ElementKind]| {
        for &parent in parents {
            for &child in children {
                table.insert((parent, child));
            }
        }
    };
    // 1-4. ModelElement `description`, `implementationConstraint`, `note`,
    // `taggedValue` (§8.6).
    admit(&MODEL_ELEMENTS, &UTILITY_ELEMENTS);
    // 5-8. AssuranceCasePackage `assuranceCasePackage`, `argumentPackage`,
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
    // 9. ArgumentPackage `argumentElement` (§11.4).
    admit(&ARGUMENT_PACKAGES, &ARGUMENTATION_ELEMENTS);
    // 10. TerminologyPackage `terminologyElement` (§10.4).
    admit(&TERMINOLOGY_PACKAGES, &TERMINOLOGY_ELEMENTS);
    // 11. ArtifactPackage `artifactElement`: every ArtifactElement (§12.2;
    // ADR 0014).
    admit(&ARTIFACT_PACKAGES, &MODEL_ELEMENTS);
    // 12. ArtifactAsset `property` (§12.6).
    admit(&PROPERTY_HOLDERS, &[Property]);
    table
}

// --- Helpers ----------------------------------------------------------------

fn kinds() -> impl Iterator<Item = ElementKind> {
    ElementKind::ALL.iter().copied()
}

/// The refusal `insert` must give for `id` of `kind` under `parent` of
/// `parent_kind`.
fn not_allowed(id: &str, kind: ElementKind, parent: &str, parent_kind: ElementKind) -> InsertError {
    InsertError::ContainmentNotAllowed {
        id: ElementId::from(id),
        kind,
        parent: ElementId::from(parent),
        parent_kind,
    }
}

/// Inserts an element, failing the test if the seam refuses it.
fn add(doc: &mut Document, parent: Option<&str>, kind: ElementKind, id: &str) {
    let parent = parent.map(ElementId::from);
    let result = doc.insert(parent.as_ref(), kind, ElementId::from(id), None);
    assert_eq!(
        result,
        Ok(()),
        "LIB-API-008: setup could not insert the {kind} {id:?} under {parent:?}"
    );
}

/// Inserts `child` (id "child") under a fresh root of `parent` (id "parent")
/// and returns what `insert` answered for the child.
fn insert_under(parent: ElementKind, child: ElementKind) -> Result<(), InsertError> {
    let mut doc = Document::new();
    add(&mut doc, None, parent, "parent");
    doc.insert(
        Some(&ElementId::from("parent")),
        child,
        ElementId::from("child"),
        None,
    )
}

/// The answer `insert_under(parent, child)` must give when the pair is legal
/// exactly if `legal`.
fn expected_answer(
    parent: ElementKind,
    child: ElementKind,
    legal: bool,
) -> Result<(), InsertError> {
    if legal {
        Ok(())
    } else {
        Err(not_allowed("child", child, "parent", parent))
    }
}

/// Asserts that `parent` accepts each of `children` and refuses each of
/// `refused`, each with the exact answer.
fn assert_admits(
    what: &str,
    parent: ElementKind,
    children: &[ElementKind],
    refused: &[ElementKind],
) {
    for &child in children {
        assert_eq!(
            insert_under(parent, child),
            Ok(()),
            "LIB-API-008: {what}: a {child} under the {parent} parent was refused"
        );
    }
    for &child in refused {
        assert_eq!(
            insert_under(parent, child),
            expected_answer(parent, child, false),
            "LIB-API-008: {what}: a {child} under the {parent} parent was not refused with ContainmentNotAllowed"
        );
    }
}

/// The kinds `parent` admits, asked of `insert` one child at a time. Any
/// answer but `Ok` or the exact `ContainmentNotAllowed` fails the test.
fn admitted_by_insert(parent: ElementKind) -> BTreeSet<ElementKind> {
    let mut admitted = BTreeSet::new();
    for child in kinds() {
        match insert_under(parent, child) {
            Ok(()) => {
                admitted.insert(child);
            }
            Err(error) => assert_eq!(
                error,
                not_allowed("child", child, "parent", parent),
                "LIB-API-008: a {child} under a {parent} was refused with the wrong error"
            ),
        }
    }
    admitted
}

/// Everything navigation can observe about a document.
#[derive(Debug, PartialEq)]
struct Snapshot {
    element_count: usize,
    roots: Vec<String>,
    elements: Vec<(String, ElementKind, Option<String>, Vec<String>)>,
}

fn snapshot(doc: &Document) -> Snapshot {
    Snapshot {
        element_count: doc.element_count(),
        roots: doc.roots().map(|root| root.id().to_string()).collect(),
        elements: doc
            .elements()
            .map(|element| {
                (
                    element.id().to_string(),
                    element.kind(),
                    element.parent().map(|parent| parent.id().to_string()),
                    element
                        .children()
                        .map(|child| child.id().to_string())
                        .collect(),
                )
            })
            .collect(),
    }
}

/// The root id used for a parent of `kind` in multi-parent documents.
fn root_id(kind: ElementKind) -> String {
    format!("root_{kind}")
}

// --- 1. The exhaustive table ---------------------------------------------

#[test]
fn lib_api_008_the_spelled_out_table_has_339_of_1444_pairs() {
    // Guards the hand-written table before it is used as the oracle.
    let all: BTreeSet<ElementKind> = kinds().collect();
    let spelled: BTreeSet<ElementKind> = MODEL_ELEMENTS
        .iter()
        .chain(&UTILITY_ELEMENTS)
        .copied()
        .collect();
    assert_eq!(
        (ElementKind::ALL.len(), all.len()),
        (38, 38),
        "LIB-API-008: ElementKind::ALL does not hold 38 distinct kinds"
    );
    assert_eq!(
        spelled, all,
        "LIB-API-008: the spelled-out ModelElement and UtilityElement kinds are not the 38 kinds"
    );
    let table = containment_table();
    assert_eq!(
        (table.len(), all.len() * all.len()),
        (LEGAL_PAIRS, 1_444),
        "LIB-API-008: the spelled-out containment table does not have 339 of 1,444 pairs"
    );
}

#[test]
fn lib_api_008_insert_admits_exactly_the_339_legal_pairs_of_the_table() {
    let table = containment_table();
    let mut mismatches = Vec::new();
    let (mut accepted, mut refused) = (0_usize, 0_usize);
    for parent in kinds() {
        for child in kinds() {
            let legal = table.contains(&(parent, child));
            let answer = insert_under(parent, child);
            if answer.is_ok() {
                accepted += 1;
            } else {
                refused += 1;
            }
            let expected = expected_answer(parent, child, legal);
            if answer != expected {
                mismatches.push(format!(
                    "{child} under {parent}: {answer:?}, expected {expected:?}"
                ));
            }
        }
    }
    assert!(
        mismatches.is_empty(),
        "LIB-API-008: {} of 1,444 pairs are answered wrongly, the first: {:#?}",
        mismatches.len(),
        &mismatches[..mismatches.len().min(20)]
    );
    assert_eq!(
        (accepted, refused),
        (LEGAL_PAIRS, 1_105),
        "LIB-API-008: insert did not accept 339 and refuse 1,105 pairs"
    );
}

// --- 2. A refusal leaves the document unchanged -------------------------------

#[test]
fn lib_api_008_containment_refusal_leaves_the_document_unchanged() {
    let table = containment_table();
    // One root of every kind, and a few legal children, so the parents'
    // children() lists are not all empty.
    let mut doc = Document::new();
    for kind in kinds() {
        add(&mut doc, None, kind, &root_id(kind));
    }
    add(&mut doc, Some(&root_id(ArgumentPackage)), Claim, "claim");
    add(&mut doc, Some(&root_id(Claim)), Note, "note");
    add(
        &mut doc,
        Some(&root_id(AssuranceCasePackage)),
        TerminologyPackage,
        "terms",
    );
    let before = snapshot(&doc);

    let mut refusals = 0_usize;
    for parent in kinds() {
        let parent_id = root_id(parent);
        for child in kinds().filter(|&child| !table.contains(&(parent, child))) {
            let answer = doc.insert(
                Some(&ElementId::from(parent_id.as_str())),
                child,
                ElementId::from("intruder"),
                None,
            );
            assert_eq!(
                answer,
                Err(not_allowed("intruder", child, &parent_id, parent)),
                "LIB-API-008: a {child} under a {parent} was not refused with ContainmentNotAllowed"
            );
            assert!(
                !doc.contains("intruder") && doc.get("intruder").is_none(),
                "LIB-API-008: a refused {child} under a {parent} is in the document"
            );
            assert_eq!(
                snapshot(&doc),
                before,
                "LIB-API-008: a refused {child} under a {parent} changed the document"
            );
            refusals += 1;
        }
    }
    assert_eq!(
        refusals, 1_105,
        "LIB-API-008: the number of illegal pairs tried is not 1,105"
    );

    // A refused id is not reserved.
    add(&mut doc, Some(&root_id(ArgumentPackage)), Claim, "intruder");
    assert_eq!(
        doc.element_count(),
        before.element_count + 1,
        "LIB-API-008: a legal insert after the refusals was not counted"
    );
}

#[test]
fn lib_api_008_containment_not_allowed_display_is_exact() {
    let mut doc = Document::new();
    add(&mut doc, None, AssuranceCasePackage, "case");
    let answer = doc.insert(
        Some(&ElementId::from("case")),
        Claim,
        ElementId::from("c1"),
        None,
    );
    assert_eq!(
        answer,
        Err(not_allowed("c1", Claim, "case", AssuranceCasePackage)),
        "LIB-API-008: a Claim directly under an AssuranceCasePackage was not refused"
    );
    assert_eq!(
        not_allowed("c1", Claim, "case", AssuranceCasePackage).to_string(),
        "element \"c1\" of kind Claim cannot be contained in element \"case\" of kind AssuranceCasePackage",
        "LIB-API-008: the ContainmentNotAllowed message is wrong"
    );
}

#[test]
fn lib_api_008_containment_not_allowed_display_quotes_both_ids_on_one_line() {
    let ids = [
        "c1",
        "Claim.",
        "a\nb",
        "\r\nleading",
        "a\u{0}b",
        "ünï",
        "UPPER",
    ];
    // Each id is the child once and the parent once, never with itself: the
    // parents are the ids rotated by one.
    for (child_id, parent_id) in ids.iter().zip(ids.iter().cycle().skip(1)) {
        let mut doc = Document::new();
        add(&mut doc, None, Note, parent_id);
        let Err(error) = doc.insert(
            Some(&ElementId::from(*parent_id)),
            Term,
            ElementId::from(*child_id),
            None,
        ) else {
            panic!("LIB-API-008: a Term under a Note was accepted");
        };
        assert_eq!(
            error,
            not_allowed(child_id, Term, parent_id, Note),
            "LIB-API-008: a Term {child_id:?} under the Note {parent_id:?} is not ContainmentNotAllowed"
        );
        let message = error.to_string();
        assert_eq!(
            message,
            format!(
                "element {child_id:?} of kind Term cannot be contained in element {parent_id:?} of kind Note"
            ),
            "LIB-API-008: the ContainmentNotAllowed message for {child_id:?} under {parent_id:?} is wrong"
        );
        assert!(
            !message.contains(['\n', '\r'])
                && message.starts_with(char::is_lowercase)
                && !message.ends_with(['.', '!', ':']),
            "LIB-API-008: the ContainmentNotAllowed message breaks the ADR 0011 rules: {message:?}"
        );
    }
}

// --- 3. Precedence ----------------------------------------------------------

#[test]
fn lib_api_008_earlier_refusals_take_precedence_over_containment() {
    let mut doc = Document::new();
    add(&mut doc, None, AssuranceCasePackage, "case");
    add(&mut doc, Some("case"), ArgumentPackage, "arguments");
    add(&mut doc, None, Note, "note");
    let before = snapshot(&doc);
    let case = ElementId::from("case");
    let note = ElementId::from("note");
    let nowhere = ElementId::from("nowhere");
    let named = || Some(LangString::new("n"));

    // Each is an illegal pair: a Claim directly under an AssuranceCasePackage,
    // a Description under a Note.
    let cases: [Attempt<'_>; 6] = [
        (
            "EmptyId",
            Some(&case),
            Claim,
            "",
            None,
            InsertError::EmptyId,
        ),
        (
            "EmptyId",
            Some(&note),
            Description,
            "",
            named(),
            InsertError::EmptyId,
        ),
        (
            "DuplicateId",
            Some(&case),
            Claim,
            "arguments",
            None,
            InsertError::DuplicateId(ElementId::from("arguments")),
        ),
        (
            "DuplicateId",
            Some(&note),
            Description,
            "case",
            named(),
            InsertError::DuplicateId(ElementId::from("case")),
        ),
        (
            "UnknownParent",
            Some(&nowhere),
            Claim,
            "orphan",
            None,
            InsertError::UnknownParent(ElementId::from("nowhere")),
        ),
        (
            "NameNotAllowed",
            Some(&note),
            Description,
            "described",
            named(),
            InsertError::NameNotAllowed(ElementId::from("described"), Description),
        ),
    ];
    for (variant, parent, kind, id, name, expected) in cases {
        assert_eq!(
            doc.insert(parent, kind, ElementId::from(id), name),
            Err(expected),
            "LIB-API-008: a {kind} {id:?} under {parent:?} is not {variant}"
        );
    }
    assert_eq!(
        snapshot(&doc),
        before,
        "LIB-API-008: the refused inserts changed the document"
    );

    // When all four earlier checks pass, the containment refusal is reported.
    assert_eq!(
        doc.insert(Some(&case), Claim, ElementId::from("c1"), None),
        Err(not_allowed("c1", Claim, "case", AssuranceCasePackage)),
        "LIB-API-008: a Claim under an AssuranceCasePackage is not ContainmentNotAllowed"
    );
    assert_eq!(
        doc.insert(Some(&note), Description, ElementId::from("d1"), None),
        Err(not_allowed("d1", Description, "note", Note)),
        "LIB-API-008: a Description under a Note is not ContainmentNotAllowed"
    );
}

/// An insert attempt: the variant it must be refused with, the insert's
/// arguments and the expected error.
type Attempt<'a> = (
    &'static str,
    Option<&'a ElementId>,
    ElementKind,
    &'static str,
    Option<LangString>,
    InsertError,
);

// --- 4. Roots are unrestricted ------------------------------------------------

#[test]
fn lib_api_008_every_kind_is_accepted_as_a_root() {
    for kind in kinds() {
        let mut doc = Document::new();
        assert_eq!(
            doc.insert(None, kind, ElementId::from("root"), None),
            Ok(()),
            "LIB-API-008: a {kind} was refused as a root"
        );
    }
    let mut doc = Document::new();
    for kind in kinds() {
        add(&mut doc, None, kind, &root_id(kind));
    }
    let roots: Vec<ElementKind> = doc.roots().map(SacmElement::kind).collect();
    assert_eq!(
        roots,
        ElementKind::ALL,
        "LIB-API-008: 38 roots of every kind, Note and Claim included, are not all held in order"
    );
}

// --- 5. UtilityElements ---------------------------------------------------------

#[test]
fn lib_api_008_utility_elements_go_under_every_model_element_and_contain_nothing() {
    for parent in MODEL_ELEMENTS {
        assert_admits("ModelElement utility roles", parent, &UTILITY_ELEMENTS, &[]);
    }
    let every_kind: Vec<ElementKind> = kinds().collect();
    for parent in UTILITY_ELEMENTS {
        assert_admits(
            "a UtilityElement contains nothing",
            parent,
            &[],
            &every_kind,
        );
    }
}

// --- 6. ArtifactPackage admits every ArtifactElement ------------------------------

#[test]
fn lib_api_008_artifact_packages_accept_every_model_element_kind() {
    for parent in ARTIFACT_PACKAGES {
        assert_admits("artifactElement (ADR 0014)", parent, &MODEL_ELEMENTS, &[]);
        assert_admits(
            "artifactElement (ADR 0014)",
            parent,
            &[AssuranceCasePackage, ArgumentPackage, Claim, Term],
            &[],
        );
    }
}

// --- 7. Inheritance --------------------------------------------------------------

#[test]
fn lib_api_008_package_interfaces_and_bindings_hold_their_package_roles() {
    assert_admits(
        "AssuranceCasePackageBinding inherits terminologyPackage",
        AssuranceCasePackageBinding,
        &[TerminologyPackage],
        &[Claim, Term],
    );
    assert_admits(
        "ArgumentPackageInterface inherits argumentElement",
        ArgumentPackageInterface,
        &[Claim],
        &[Term, AssuranceCasePackage],
    );
    for family in [
        ASSURANCE_CASE_PACKAGES,
        ARGUMENT_PACKAGES,
        ARTIFACT_PACKAGES,
        TERMINOLOGY_PACKAGES,
    ] {
        let package = admitted_by_insert(family[0]);
        for member in &family[1..] {
            assert_eq!(
                admitted_by_insert(*member),
                package,
                "LIB-API-008: a {member} does not admit what its package {} admits",
                family[0]
            );
        }
    }
}

#[test]
fn lib_api_008_artifact_asset_relationship_and_property_accept_property() {
    for parent in [ArtifactAssetRelationship, Property] {
        assert_admits(
            "ArtifactAsset property (§12.6, §12.14 via D-TEXT-MODEL)",
            parent,
            &[Property],
            &[Artifact, ArtifactAssetRelationship, Claim],
        );
    }
}

#[test]
fn lib_api_008_groups_accept_only_utility_elements() {
    let others: Vec<ElementKind> = MODEL_ELEMENTS.to_vec();
    for group in [ArgumentGroup, ArtifactGroup, TerminologyGroup] {
        assert_admits(
            "group members are references, not containment",
            group,
            &UTILITY_ELEMENTS,
            &others,
        );
    }
}

#[test]
fn lib_api_008_terminology_package_accepts_terminology_group() {
    assert_admits(
        "terminologyElement admits TerminologyGroup (spec defect D9)",
        TerminologyPackage,
        &[TerminologyGroup],
        &[ArgumentGroup, ArtifactGroup, Claim],
    );
}

// --- 8. Multiplicity is not enforced -------------------------------------------

#[test]
fn lib_api_008_a_second_and_third_description_under_one_claim_are_accepted() {
    let mut doc = Document::new();
    add(&mut doc, None, Claim, "claim");
    for id in ["description_1", "description_2", "description_3"] {
        add(&mut doc, Some("claim"), Description, id);
    }
    let Some(claim) = doc.get("claim") else {
        panic!("LIB-API-008: the claim is missing");
    };
    let children: Vec<&str> = claim.children().map(|child| child.id().as_str()).collect();
    assert_eq!(
        children,
        ["description_1", "description_2", "description_3"],
        "LIB-API-008: three Descriptions under one Claim are not all held (D-DESC-MULT)"
    );
}

#[test]
fn lib_api_008_any_number_of_children_of_any_role_is_accepted() {
    const COPIES: usize = 25;
    let table = containment_table();
    for parent in [
        AssuranceCasePackage,
        ArgumentPackage,
        TerminologyPackage,
        ArtifactPackage,
        Artifact,
    ] {
        let legal: Vec<ElementKind> = kinds()
            .filter(|&child| table.contains(&(parent, child)))
            .collect();
        let mut doc = Document::new();
        add(&mut doc, None, parent, "parent");
        let mut expected = Vec::new();
        // Interleaved: one of every legal kind, COPIES times over.
        for copy in 0..COPIES {
            for &child in &legal {
                let id = format!("{child}_{copy}");
                add(&mut doc, Some("parent"), child, &id);
                expected.push(id);
            }
        }
        let Some(view) = doc.get("parent") else {
            panic!("LIB-API-008: the {parent} is missing");
        };
        let children: Vec<String> = view
            .children()
            .map(|child| child.id().to_string())
            .collect();
        assert_eq!(
            children,
            expected,
            "LIB-API-008: {COPIES} children of each of {} kinds under a {parent} are not all held in order",
            legal.len()
        );
    }
}

// --- 9. Drift test against the inventory ------------------------------------------

/// The inventory's classes that are values, not `SACMElement`s, and so have
/// no kind. Rows for them in the containment table are not parents.
const VALUE_TYPES: [&str; 3] = ["LangString", "MultiLangString", "ExpressionLangString"];

/// The value-typed composite roles: their type is a value type, not an
/// element kind, so they contain no elements and are excluded from the drift
/// comparison. `location` (Resource, §12.10) is followed from the text
/// and is not in the ptc-derived table today.
const VALUE_ROLES: [&str; 5] = ["name", "value", "key", "content", "location"];

/// One row of the inventory's "Concrete class -> containment roles" table.
struct InventoryRow {
    class: String,
    /// `(role, child type)` pairs.
    roles: Vec<(String, String)>,
}

fn inventory_text() -> String {
    let path = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../docs/sacm/metamodel-inventory.md");
    match std::fs::read_to_string(&path) {
        Ok(text) => text,
        Err(error) => panic!("LIB-API-008: cannot read {}: {error}", path.display()),
    }
}

/// Parses the table under `## Concrete class -> containment roles`:
///
/// ```text
/// | Concrete class | Package | Containment roles (role: child type) |
/// | --- | --- | --- |
/// | Claim | Argumentation | `description`: Description; ...; `name`: LangString |
/// | LangString | Base | (none) |
/// ```
fn containment_roles_table(text: &str) -> Vec<InventoryRow> {
    let mut rows = Vec::new();
    let mut in_section = false;
    for line in text.lines() {
        if let Some(heading) = line.strip_prefix("## ") {
            in_section = heading.starts_with("Concrete class -> containment roles");
            continue;
        }
        if !in_section || !line.starts_with('|') {
            continue;
        }
        let cells: Vec<&str> = line.trim_matches('|').split('|').map(str::trim).collect();
        let [class, _package, roles] = cells.as_slice() else {
            panic!("LIB-API-008: an inventory containment row does not have three cells: {line:?}");
        };
        if *class == "Concrete class" || class.starts_with("---") {
            continue;
        }
        let roles = if *roles == "(none)" {
            Vec::new()
        } else {
            roles
                .split(';')
                .map(|role| parse_role(class, role.trim()))
                .collect()
        };
        rows.push(InventoryRow {
            class: (*class).to_owned(),
            roles,
        });
    }
    rows
}

/// Parses "`role`: Type".
fn parse_role(class: &str, role: &str) -> (String, String) {
    let parsed = role
        .strip_prefix('`')
        .and_then(|rest| rest.split_once("`: "))
        .map(|(name, child)| (name.to_owned(), child.trim().to_owned()));
    let Some(parsed) = parsed else {
        panic!("LIB-API-008: the {class} role {role:?} is not \"`role`: Type\"");
    };
    parsed
}

fn kind_named(name: &str) -> Option<ElementKind> {
    kinds().find(|kind| kind.name() == name)
}

fn class_named(name: &str) -> Option<AbstractClass> {
    AbstractClass::ALL
        .iter()
        .copied()
        .find(|class| class.name() == name)
}

/// The kinds a role of declared type `child` admits: a concrete type by
/// `is_kind_of`, an abstract one by `is_a`.
fn admitted_by_type(class: &str, role: &str, child: &str) -> BTreeSet<ElementKind> {
    if let Some(declared) = kind_named(child) {
        kinds().filter(|kind| kind.is_kind_of(declared)).collect()
    } else if let Some(declared) = class_named(child) {
        kinds().filter(|kind| kind.is_a(declared)).collect()
    } else {
        panic!(
            "LIB-API-008: the type {child:?} of {class}.{role} is neither an element kind nor an abstract class"
        );
    }
}

/// The kinds each concrete class admits per the inventory, and the
/// value-typed roles excluded on the way, as `class.role`.
fn inventory_admits() -> (Vec<(ElementKind, BTreeSet<ElementKind>)>, BTreeSet<String>) {
    let rows = containment_roles_table(&inventory_text());
    let mut parents = Vec::new();
    let mut excluded = BTreeSet::new();
    for row in rows {
        if VALUE_TYPES.contains(&row.class.as_str()) {
            // A value type is not an element and is never a parent, but its
            // own value-typed roles (MultiLangString `value`) are listed.
            for (role, child) in &row.roles {
                assert!(
                    VALUE_TYPES.contains(&child.as_str()) && VALUE_ROLES.contains(&role.as_str()),
                    "LIB-API-008: the value type {}.{role} has the type {child}, which is not a value",
                    row.class
                );
                excluded.insert(format!("{}.{role}", row.class));
            }
            continue;
        }
        let Some(parent) = kind_named(&row.class) else {
            panic!(
                "LIB-API-008: the inventory class {} is not an element kind",
                row.class
            );
        };
        let mut admitted = BTreeSet::new();
        for (role, child) in &row.roles {
            if VALUE_TYPES.contains(&child.as_str()) {
                assert!(
                    VALUE_ROLES.contains(&role.as_str()),
                    "LIB-API-008: {}.{role} is value-typed ({child}) but not a known value role",
                    row.class
                );
                excluded.insert(format!("{}.{role}", row.class));
            } else {
                assert!(
                    !VALUE_ROLES.contains(&role.as_str()),
                    "LIB-API-008: the value role {}.{role} has the element type {child}",
                    row.class
                );
                admitted.extend(admitted_by_type(&row.class, role, child));
            }
        }
        parents.push((parent, admitted));
    }
    (parents, excluded)
}

#[test]
fn lib_api_008_inventory_containment_table_parses_into_every_kind() {
    // Guards the parser: a format change must fail here, not compare nothing.
    let rows = containment_roles_table(&inventory_text());
    let classes: Vec<&str> = rows.iter().map(|row| row.class.as_str()).collect();
    let value_rows: BTreeSet<&str> = classes
        .iter()
        .copied()
        .filter(|class| VALUE_TYPES.contains(class))
        .collect();
    assert_eq!(
        value_rows,
        BTreeSet::from(VALUE_TYPES),
        "LIB-API-008: the inventory table's value-type rows are not LangString, MultiLangString and ExpressionLangString"
    );
    let (parents, excluded) = inventory_admits();
    let parent_kinds: Vec<ElementKind> = parents.iter().map(|(kind, _)| *kind).collect();
    let unique: BTreeSet<ElementKind> = parent_kinds.iter().copied().collect();
    assert_eq!(
        (parent_kinds.len(), unique, classes.len()),
        (38, kinds().collect(), 41),
        "LIB-API-008: the inventory table does not list each of the 38 kinds once, plus 3 value types"
    );
    // The value-typed roles excluded from the comparison, listed as such.
    let names: BTreeSet<&str> = excluded
        .iter()
        .filter_map(|role| role.split_once('.').map(|(_, name)| name))
        .collect();
    assert_eq!(
        names,
        BTreeSet::from(["content", "key", "name", "value"]),
        "LIB-API-008: the excluded value-typed roles are not name, value, key and content: {excluded:?}"
    );
    assert_eq!(
        excluded.len(),
        34 + 4 + 1 + 1,
        "LIB-API-008: expected 34 `name`, 4 UtilityElement `content`, TaggedValue `key` and MultiLangString `value`: {excluded:?}"
    );
    let row = |name: &str| rows.iter().find(|row| row.class == name);
    assert!(
        row("ArtifactAssetRelationship").is_some_and(|row| row
            .roles
            .iter()
            .any(|(role, child)| role == "property" && child == "Property")),
        "LIB-API-008: the inventory does not give ArtifactAssetRelationship `property` (D-TEXT-MODEL)"
    );
}

#[test]
fn lib_api_008_insert_admits_exactly_what_the_inventory_roles_admit() {
    let (parents, _) = inventory_admits();
    let total: usize = parents.iter().map(|(_, admitted)| admitted.len()).sum();
    assert_eq!(
        total, LEGAL_PAIRS,
        "LIB-API-008: the inventory's element-typed roles do not give 339 legal pairs"
    );
    let mut drift = Vec::new();
    for (parent, expected) in parents {
        let actual = admitted_by_insert(parent);
        if actual != expected {
            let extra: Vec<_> = actual.difference(&expected).collect();
            let missing: Vec<_> = expected.difference(&actual).collect();
            drift.push(format!(
                "{parent}: admits {extra:?} beyond the inventory, misses {missing:?}"
            ));
        }
    }
    assert!(
        drift.is_empty(),
        "LIB-API-008: insert drifts from the inventory under {} classes: {drift:#?}",
        drift.len()
    );
}
