//! Tests for AssuranceCasePackage and its nesting (SACM23-AC-001,
//! formal/23-05-08 §9.2, ADR 0010 D2).
//!
//! An AssuranceCasePackage is a `Base::ArtifactElement`. It is held as a root
//! or nested, and contains sub-packages through its four composition roles,
//! each `0..*`: `assuranceCasePackage`, `argumentPackage`, `artifactPackage`
//! and `terminologyPackage`. A sub-package's role follows from its kind, and
//! children keep document order across roles.
//!
//! Which kinds may be contained, and the refusal of the others, is LIB-API-008
//! (`tests/containment.rs`). The `interface` reference is SACM23-AC-002.
//!
//! Documents are built through `Document::insert`, the `test-support` seam. The
//! crate dev-depends on itself with that feature, so these tests always see it.

use sacm::model::{AbstractClass, Document, ElementId, ElementKind, MetamodelPackage, SacmElement};

/// Inserts an element, failing the test if the seam refuses it.
fn add(doc: &mut Document, parent: Option<&str>, kind: ElementKind, id: &str) {
    let parent = parent.map(ElementId::from);
    let result = doc.insert(parent.as_ref(), kind, ElementId::from(id), None);
    assert_eq!(
        result,
        Ok(()),
        "SACM23-AC-001: setup could not insert the {kind} {id:?} under {parent:?}"
    );
}

/// Looks up an element that the test expects to exist.
fn view<'d>(doc: &'d Document, id: &str) -> SacmElement<'d> {
    let Some(element) = doc.get(id) else {
        panic!("SACM23-AC-001: {id:?} is missing from the document");
    };
    element
}

fn ids<'a>(views: impl Iterator<Item = SacmElement<'a>>) -> Vec<String> {
    views.map(|element| element.id().to_string()).collect()
}

fn parent_id(element: SacmElement<'_>) -> Option<String> {
    element.parent().map(|parent| parent.id().to_string())
}

// --- 1. The kind ---------------------------------------------------------------

#[test]
fn sacm23_ac_001_assurance_case_package_kind_is_an_artifact_element_of_assurance_case() {
    let kind = ElementKind::AssuranceCasePackage;
    assert_eq!(
        (kind.name(), kind.to_string()),
        ("AssuranceCasePackage", "AssuranceCasePackage".to_owned()),
        "SACM23-AC-001: the kind is not spelled AssuranceCasePackage"
    );
    assert_eq!(
        kind.package(),
        MetamodelPackage::AssuranceCase,
        "SACM23-AC-001: the kind is not in the AssuranceCase package"
    );
    for class in [
        AbstractClass::ArtifactElement,
        AbstractClass::ModelElement,
        AbstractClass::SACMElement,
    ] {
        assert!(
            kind.is_a(class),
            "SACM23-AC-001: AssuranceCasePackage is not a {class} (§9.2 \"Superclass Base::ArtifactElement\")"
        );
    }
    for class in [
        AbstractClass::ArgumentationElement,
        AbstractClass::TerminologyElement,
        AbstractClass::UtilityElement,
    ] {
        assert!(
            !kind.is_a(class),
            "SACM23-AC-001: AssuranceCasePackage is a {class}"
        );
    }
}

// --- 2. Root or nested ----------------------------------------------------------

#[test]
fn sacm23_ac_001_root_assurance_case_package_is_in_roots_without_a_parent() {
    let mut doc = Document::new();
    add(&mut doc, None, ElementKind::AssuranceCasePackage, "case");
    assert_eq!(
        ids(doc.roots()),
        ["case"],
        "SACM23-AC-001: a root AssuranceCasePackage is not in roots()"
    );
    let case = view(&doc, "case");
    assert_eq!(
        (case.kind(), parent_id(case)),
        (ElementKind::AssuranceCasePackage, None),
        "SACM23-AC-001: a root AssuranceCasePackage has the wrong kind or a parent"
    );
}

#[test]
fn sacm23_ac_001_three_nested_assurance_case_packages_chain_through_parent_and_children() {
    let mut doc = Document::new();
    add(&mut doc, None, ElementKind::AssuranceCasePackage, "outer");
    add(
        &mut doc,
        Some("outer"),
        ElementKind::AssuranceCasePackage,
        "middle",
    );
    add(
        &mut doc,
        Some("middle"),
        ElementKind::AssuranceCasePackage,
        "inner",
    );

    assert_eq!(
        ids(doc.roots()),
        ["outer"],
        "SACM23-AC-001: a nested AssuranceCasePackage is in roots()"
    );
    for (inner, outer) in [("middle", "outer"), ("inner", "middle")] {
        let element = view(&doc, inner);
        assert_eq!(
            parent_id(element).as_deref(),
            Some(outer),
            "SACM23-AC-001: the parent of {inner} is not {outer}"
        );
        assert_eq!(
            ids(view(&doc, outer).children()),
            [inner],
            "SACM23-AC-001: {inner} is not the child of {outer}"
        );
    }
    assert_eq!(
        view(&doc, "inner").children().count(),
        0,
        "SACM23-AC-001: the innermost package has children it was never given"
    );
}

// --- 3. Order across roles -------------------------------------------------------

/// Children of one AssuranceCasePackage from all four roles, interleaved, in
/// insertion order.
const INTERLEAVED: [(ElementKind, &str); 7] = [
    (ElementKind::ArgumentPackage, "a1"),
    (ElementKind::AssuranceCasePackage, "n1"),
    (ElementKind::ArtifactPackage, "f1"),
    (ElementKind::ArgumentPackageInterface, "a2"),
    (ElementKind::TerminologyPackage, "t1"),
    (ElementKind::AssuranceCasePackageBinding, "n2"),
    (ElementKind::TerminologyPackageInterface, "t2"),
];

fn interleaved() -> Document {
    let mut doc = Document::new();
    add(&mut doc, None, ElementKind::AssuranceCasePackage, "case");
    for (kind, id) in INTERLEAVED {
        add(&mut doc, Some("case"), kind, id);
    }
    // A subtree under n1, so elements() shows that n1's subtree comes first.
    add(
        &mut doc,
        Some("n1"),
        ElementKind::ArgumentPackage,
        "n1_arguments",
    );
    doc
}

#[test]
fn sacm23_ac_001_children_from_all_four_roles_keep_insertion_order() {
    let doc = interleaved();
    let case = view(&doc, "case");
    assert_eq!(
        ids(case.children()),
        ["a1", "n1", "f1", "a2", "t1", "n2", "t2"],
        "SACM23-AC-001: children from the four roles are not in insertion order (ADR 0010 D2)"
    );
    for child in case.children() {
        assert_eq!(
            parent_id(child).as_deref(),
            Some("case"),
            "SACM23-AC-001: {} does not have the package as its parent",
            child.id()
        );
    }
    let kinds: Vec<ElementKind> = case.children().map(SacmElement::kind).collect();
    let expected: Vec<ElementKind> = INTERLEAVED.iter().map(|(kind, _)| *kind).collect();
    assert_eq!(
        kinds, expected,
        "SACM23-AC-001: the children report the wrong kinds"
    );
}

#[test]
fn sacm23_ac_001_filtering_children_by_role_type_gives_each_role_in_order() {
    let doc = interleaved();
    let case = view(&doc, "case");
    let role = |declared: ElementKind| {
        ids(case
            .children()
            .filter(|child| child.kind().is_kind_of(declared)))
    };
    assert_eq!(
        [
            role(ElementKind::ArgumentPackage),
            role(ElementKind::AssuranceCasePackage),
            role(ElementKind::ArtifactPackage),
            role(ElementKind::TerminologyPackage),
        ],
        [
            vec!["a1", "a2"],
            vec!["n1", "n2"],
            vec!["f1"],
            vec!["t1", "t2"],
        ],
        "SACM23-AC-001: the argumentPackage / assuranceCasePackage / artifactPackage / terminologyPackage children are wrong"
    );
}

#[test]
fn sacm23_ac_001_elements_visit_a_sub_package_subtree_before_the_next_sibling() {
    assert_eq!(
        ids(interleaved().elements()),
        [
            "case",
            "a1",
            "n1",
            "n1_arguments",
            "f1",
            "a2",
            "t1",
            "n2",
            "t2"
        ],
        "SACM23-AC-001: elements() does not visit n1's subtree before f1"
    );
}

// --- 4. Multiplicity 0..* ---------------------------------------------------------

#[test]
fn sacm23_ac_001_assurance_case_package_without_children_is_held() {
    let mut doc = Document::new();
    add(&mut doc, None, ElementKind::AssuranceCasePackage, "empty");
    assert_eq!(
        (doc.element_count(), view(&doc, "empty").children().count()),
        (1, 0),
        "SACM23-AC-001: an AssuranceCasePackage with no children is not held as such"
    );
}

#[test]
fn sacm23_ac_001_a_thousand_argument_packages_are_kept_in_insertion_order() {
    const COUNT: usize = 1_000;
    // Ids in an order that is not their sort order.
    let id_at = |index: usize| format!("arguments_{}", (index * 7919) % COUNT);
    let mut doc = Document::new();
    add(&mut doc, None, ElementKind::AssuranceCasePackage, "case");
    for index in 0..COUNT {
        add(
            &mut doc,
            Some("case"),
            ElementKind::ArgumentPackage,
            &id_at(index),
        );
    }
    let expected: Vec<String> = (0..COUNT).map(id_at).collect();
    assert_eq!(
        ids(view(&doc, "case").children()),
        expected,
        "SACM23-AC-001: 1,000 ArgumentPackage children are not all kept in insertion order"
    );
    assert!(
        view(&doc, "case")
            .children()
            .all(|child| child.kind() == ElementKind::ArgumentPackage),
        "SACM23-AC-001: a child changed kind"
    );
}
