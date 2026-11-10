//! Tests for ArgumentPackage, as a root or nested, holding argumentation
//! elements in document order (SACM23-ARG-001, formal/23-05-08 §11.4, §2.2,
//! ADR 0010 D2).
//!
//! An ArgumentPackage is an `ArgumentationElement` (so an `ArtifactElement`,
//! §11.3). Its `argumentElement` composition (`0..*`) holds any mix of the 12
//! `ArgumentationElement` kinds, in document order and interleaved with its
//! `ModelElement` utility children.
//!
//! Which kinds are admitted in general is LIB-API-008 (`tests/containment.rs`);
//! these tests characterise the ArgumentPackage cases the row lists. The §11.4
//! homogeneity rule is a validation report (SACM23-VAL-012), never a refusal.
//!
//! Documents are built through `Document::insert`, the `test-support` seam. The
//! crate dev-depends on itself with that feature, so these tests always see it.

use sacm::model::{AbstractClass, Document, ElementId, ElementKind, InsertError, SacmElement};

/// Inserts an element, failing the test if the seam refuses it.
fn add(doc: &mut Document, parent: Option<&str>, kind: ElementKind, id: &str) {
    let parent = parent.map(ElementId::from);
    let result = doc.insert(parent.as_ref(), kind, ElementId::from(id), None);
    assert_eq!(
        result,
        Ok(()),
        "SACM23-ARG-001: setup could not insert the {kind} {id:?} under {parent:?}"
    );
}

/// Looks up an element that the test expects to exist.
fn view<'d>(doc: &'d Document, id: &str) -> SacmElement<'d> {
    let Some(element) = doc.get(id) else {
        panic!("SACM23-ARG-001: {id:?} is missing from the document");
    };
    element
}

fn ids<'a>(views: impl Iterator<Item = SacmElement<'a>>) -> Vec<String> {
    views.map(|element| element.id().to_string()).collect()
}

fn parent_id(element: SacmElement<'_>) -> Option<String> {
    element.parent().map(|parent| parent.id().to_string())
}

// --- 1. Standalone root ----------------------------------------------------------

#[test]
fn sacm23_arg_001_argument_package_is_a_standalone_root() {
    let mut doc = Document::new();
    add(&mut doc, None, ElementKind::ArgumentPackage, "arguments");

    assert!(
        doc.elements()
            .all(|element| element.kind() != ElementKind::AssuranceCasePackage),
        "SACM23-ARG-001: the document has an AssuranceCasePackage"
    );
    assert_eq!(
        ids(doc.roots()),
        ["arguments"],
        "SACM23-ARG-001: the ArgumentPackage is not the only root (§2.2)"
    );
    let package = view(&doc, "arguments");
    assert_eq!(
        package.kind(),
        ElementKind::ArgumentPackage,
        "SACM23-ARG-001: the root is not an ArgumentPackage"
    );
    assert_eq!(
        parent_id(package),
        None,
        "SACM23-ARG-001: a root ArgumentPackage has a parent"
    );
    assert_eq!(
        doc.element_count(),
        1,
        "SACM23-ARG-001: a lone ArgumentPackage is not the only element"
    );
}

// --- 2. The 12 kinds mixed under one role -----------------------------------------

/// One of each `ArgumentationElement` kind, in the row's non-model order.
const MIXED_ORDER: [ElementKind; 12] = [
    ElementKind::AssertedEvidence,
    ElementKind::Claim,
    ElementKind::ArgumentPackageBinding,
    ElementKind::ArgumentGroup,
    ElementKind::ArtifactReference,
    ElementKind::ArgumentPackage,
    ElementKind::AssertedContext,
    ElementKind::ArgumentReasoning,
    ElementKind::ArgumentPackageInterface,
    ElementKind::AssertedInference,
    ElementKind::AssertedArtifactContext,
    ElementKind::AssertedArtifactSupport,
];

#[test]
fn sacm23_arg_001_the_mixed_order_holds_each_argumentation_element_kind_once() {
    let mut listed: Vec<ElementKind> = MIXED_ORDER.to_vec();
    listed.sort_unstable();
    let mut argumentation: Vec<ElementKind> = ElementKind::ALL
        .iter()
        .copied()
        .filter(|kind| kind.is_a(AbstractClass::ArgumentationElement))
        .collect();
    argumentation.sort_unstable();
    assert_eq!(
        listed, argumentation,
        "SACM23-ARG-001: the row's 12 kinds are not exactly the ArgumentationElement kinds"
    );
}

#[test]
fn sacm23_arg_001_all_twelve_argumentation_kinds_keep_insertion_order() {
    let mut doc = Document::new();
    add(&mut doc, None, ElementKind::ArgumentPackage, "arguments");
    let expected: Vec<(String, ElementKind)> = MIXED_ORDER
        .iter()
        .enumerate()
        .map(|(index, &kind)| (format!("e{index}_{kind}"), kind))
        .collect();
    for (id, kind) in &expected {
        add(&mut doc, Some("arguments"), *kind, id);
    }

    let package = view(&doc, "arguments");
    let children: Vec<(String, ElementKind)> = package
        .children()
        .map(|child| (child.id().to_string(), child.kind()))
        .collect();
    assert_eq!(
        children, expected,
        "SACM23-ARG-001: the 12 argumentation kinds did not come back in insertion order"
    );
    for (id, kind) in &expected {
        assert_eq!(
            parent_id(view(&doc, id)).as_deref(),
            Some("arguments"),
            "SACM23-ARG-001: the {kind} {id:?} does not have the package as parent"
        );
    }
}

// --- 3. Order across roles -------------------------------------------------------

#[test]
fn sacm23_arg_001_children_interleave_argumentation_and_utility_elements_in_order() {
    let mut doc = Document::new();
    add(&mut doc, None, ElementKind::ArgumentPackage, "arguments");
    let inserted = [
        ("d1", ElementKind::Description),
        ("c1", ElementKind::Claim),
        ("n1", ElementKind::Note),
        ("r1", ElementKind::ArgumentReasoning),
        ("t1", ElementKind::TaggedValue),
        ("c2", ElementKind::Claim),
    ];
    for (id, kind) in inserted {
        add(&mut doc, Some("arguments"), kind, id);
    }
    let package = view(&doc, "arguments");

    assert_eq!(
        ids(package.children()),
        ["d1", "c1", "n1", "r1", "t1", "c2"],
        "SACM23-ARG-001: children across roles are not in document order (ADR 0010 D2)"
    );
    assert_eq!(
        ids(package
            .children()
            .filter(|child| child.kind().is_a(AbstractClass::ArgumentationElement))),
        ["c1", "r1", "c2"],
        "SACM23-ARG-001: the argumentElement children are not c1, r1, c2 in order"
    );
    assert_eq!(
        ids(package
            .children()
            .filter(|child| child.kind().is_a(AbstractClass::UtilityElement))),
        ["d1", "n1", "t1"],
        "SACM23-ARG-001: the utility children are not d1, n1, t1 in order"
    );
}

// --- 4. Nesting --------------------------------------------------------------------

#[test]
fn sacm23_arg_001_argument_packages_nest_in_a_case_and_in_each_other() {
    let mut doc = Document::new();
    add(&mut doc, None, ElementKind::AssuranceCasePackage, "case");
    add(&mut doc, Some("case"), ElementKind::ArgumentPackage, "p1");
    add(&mut doc, Some("p1"), ElementKind::ArgumentPackage, "p2");
    add(&mut doc, Some("p2"), ElementKind::ArgumentPackage, "p3");
    add(&mut doc, Some("p3"), ElementKind::Claim, "claim");

    for (inner, outer) in [("p1", "case"), ("p2", "p1"), ("p3", "p2"), ("claim", "p3")] {
        assert_eq!(
            parent_id(view(&doc, inner)).as_deref(),
            Some(outer),
            "SACM23-ARG-001: the parent of {inner:?} is not {outer:?}"
        );
        assert_eq!(
            ids(view(&doc, outer).children()),
            [inner],
            "SACM23-ARG-001: {inner:?} is not the child of {outer:?}"
        );
    }
    assert_eq!(
        ids(doc.roots()),
        ["case"],
        "SACM23-ARG-001: a nested ArgumentPackage is listed as a root"
    );
    assert_eq!(
        ids(doc.elements()),
        ["case", "p1", "p2", "p3", "claim"],
        "SACM23-ARG-001: elements() does not visit the chain in pre-order"
    );
}

// --- 5. Homogeneity is not structural ---------------------------------------------

#[test]
fn sacm23_arg_001_mixing_a_nested_package_and_a_claim_is_not_refused() {
    // §11.4: "If an ArgumentPackage has nested ArgumentPackages, then it is
    // only allowed to contain ArgumentPackages." That is SACM23-VAL-012's
    // report (D-PKG-HOMOG, D-VAL-LAYER), never a refusal on insertion.
    for (first, second) in [
        (ElementKind::ArgumentPackage, ElementKind::Claim),
        (ElementKind::Claim, ElementKind::ArgumentPackage),
    ] {
        let mut doc = Document::new();
        add(&mut doc, None, ElementKind::ArgumentPackage, "outer");
        for (id, kind) in [("first", first), ("second", second)] {
            assert_eq!(
                doc.insert(
                    Some(&ElementId::from("outer")),
                    kind,
                    ElementId::from(id),
                    None
                ),
                Ok(()),
                "SACM23-ARG-001: a {kind} {id:?} after a {first} was refused (homogeneity is VAL-012)"
            );
        }
        let held: Vec<(String, ElementKind)> = view(&doc, "outer")
            .children()
            .map(|child| (child.id().to_string(), child.kind()))
            .collect();
        assert_eq!(
            held,
            [("first".to_owned(), first), ("second".to_owned(), second)],
            "SACM23-ARG-001: a {first} then a {second} are not both held in order"
        );
    }
}

// --- 6. Negative containment -------------------------------------------------------

#[test]
fn sacm23_arg_001_non_argumentation_kinds_are_refused_under_an_argument_package() {
    let mut doc = Document::new();
    add(&mut doc, None, ElementKind::ArgumentPackage, "arguments");
    add(&mut doc, Some("arguments"), ElementKind::Claim, "c1");
    let count = doc.element_count();

    for kind in [
        ElementKind::Artifact,
        ElementKind::Term,
        ElementKind::AssuranceCasePackage,
        ElementKind::ArtifactPackage,
        ElementKind::TerminologyPackage,
    ] {
        let id = format!("refused_{kind}");
        assert_eq!(
            doc.insert(
                Some(&ElementId::from("arguments")),
                kind,
                ElementId::from(id.as_str()),
                None,
            ),
            Err(InsertError::ContainmentNotAllowed {
                id: ElementId::from(id.as_str()),
                kind,
                parent: ElementId::from("arguments"),
                parent_kind: ElementKind::ArgumentPackage,
            }),
            "SACM23-ARG-001: a {kind} under an ArgumentPackage is not ContainmentNotAllowed"
        );
        assert_eq!(
            doc.element_count(),
            count,
            "SACM23-ARG-001: a refused {kind} changed the element count"
        );
        assert!(
            !doc.contains(id.as_str()),
            "SACM23-ARG-001: a refused {kind} is in the document"
        );
    }
    assert_eq!(
        ids(view(&doc, "arguments").children()),
        ["c1"],
        "SACM23-ARG-001: refused inserts changed the package's children"
    );
}

// --- 7. 0..* is unbounded ----------------------------------------------------------

#[test]
fn sacm23_arg_001_an_empty_argument_package_has_no_children() {
    let mut doc = Document::new();
    add(&mut doc, None, ElementKind::ArgumentPackage, "empty");
    assert_eq!(
        view(&doc, "empty").children().count(),
        0,
        "SACM23-ARG-001: an empty ArgumentPackage has children"
    );
}

#[test]
fn sacm23_arg_001_one_thousand_claims_keep_insertion_order() {
    const CLAIMS: usize = 1_000;
    let mut doc = Document::new();
    add(&mut doc, None, ElementKind::ArgumentPackage, "arguments");
    let expected: Vec<String> = (0..CLAIMS).map(|index| format!("c{index}")).collect();
    for id in &expected {
        add(&mut doc, Some("arguments"), ElementKind::Claim, id);
    }

    let package = view(&doc, "arguments");
    assert_eq!(
        ids(package.children()),
        expected,
        "SACM23-ARG-001: 1,000 claims did not come back in insertion order"
    );
    assert!(
        package
            .children()
            .all(|child| child.kind() == ElementKind::Claim),
        "SACM23-ARG-001: a child of the 1,000 is not a Claim"
    );
    assert_eq!(
        doc.element_count(),
        CLAIMS + 1,
        "SACM23-ARG-001: the element count is not the package and 1,000 claims"
    );
}
