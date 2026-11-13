//! Tests for `sacm::compare::semantic_compare`, `Difference` and
//! `Document: PartialEq + Eq` (LIB-RT-001; ADR 0020, ADR 0010 D2, ADR 0012,
//! ADR 0013).
//!
//! Documents are built in two ways:
//! - with `edit::apply`, the public way, for the "equal" items;
//! - with the `test-support` seams `Document::insert` and
//!   `Document::set_assertion_declaration`, for what the edit API cannot
//!   build: a missing name, a name of `LangString::default()`, a kind change,
//!   a different parent, a reordered sibling or root, and kinds other than the
//!   three the edit API creates.
//!
//! Every "one difference" test compares the whole result with the single
//! expected `Difference`, so an extra difference fails, and checks the swapped
//! comparison and `==` in both directions as well (`assert_only`).
//!
//! Not checked here:
//! - `#[non_exhaustive]` on `Difference`: a test cannot show that an
//!   exhaustive match fails to compile. The matches here use a wildcard arm,
//!   as a user's must.
//! - That the generated-id counters are left out on purpose rather than by
//!   accident: a test can only show that documents with different counters
//!   compare equal (`lib_rt_001_generated_and_caller_ids_with_the_same_content_are_equal`).

use std::collections::HashSet;
use std::fmt::{Debug, Display};
use std::hash::Hash;

use sacm::compare::{Difference, semantic_compare};
use sacm::edit::{self, CreateArgumentPackage, CreateAssuranceCasePackage, CreateClaim, Operation};
use sacm::model::{
    AssertionDeclaration, Document, ElementId, ElementKind, LangString, MultiLangString,
};
use sacm::xmi::{self, ReadOptions};

/// The row every test here verifies.
const ROW: &str = "LIB-RT-001";

/// The three states of one `LangString` field: absent, empty and non-empty
/// (non-ASCII, so that a byte-wise or ASCII-only comparison shows).
const FIELD_STATES: [Option<&str>; 3] = [None, Some(""), Some("Säkerhet ✓ 安全")];

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

/// An element id.
fn id(value: &str) -> ElementId {
    ElementId::from(value)
}

/// Element ids, in the order given.
fn ids(values: &[&str]) -> Vec<ElementId> {
    values.iter().copied().map(ElementId::from).collect()
}

/// A name with exactly the given `lang` and `content`, each absent when `None`.
fn lang_string(lang: Option<&str>, content: Option<&str>) -> LangString {
    let mut value = LangString::default();
    value.set_lang(lang.map(str::to_owned));
    value.set_content(content.map(str::to_owned));
    value
}

/// A non-empty English name.
fn en(content: &str) -> LangString {
    LangString::new(content).with_lang("en")
}

/// Inserts one element through the `test-support` seam, or fails the test as
/// a setup error.
fn add(
    doc: &mut Document,
    parent: Option<&str>,
    kind: ElementKind,
    element: &str,
    name: Option<LangString>,
) {
    let parent = parent.map(ElementId::from);
    if let Err(error) = doc.insert(parent.as_ref(), kind, id(element), name) {
        panic!("{ROW}: setup could not insert the {kind} {element:?} under {parent:?}: {error}");
    }
}

/// Sets the declaration of the Assertion `element` through the seam, or fails
/// the test as a setup error.
fn declare(doc: &mut Document, element: &ElementId, declaration: AssertionDeclaration) {
    if let Err(error) = doc.set_assertion_declaration(element, declaration) {
        panic!("{ROW}: setup could not set {declaration} on {element:?}: {error}");
    }
}

/// Applies a create operation and returns the id of the element it created,
/// or fails the test as a setup error.
fn create(doc: &mut Document, operation: impl Into<Operation>) -> ElementId {
    let operation = operation.into();
    match edit::apply(doc, operation.clone()) {
        Ok(mutation) => match mutation.created_ids().first() {
            Some(created) => created.clone(),
            None => panic!("{ROW}: setup: {operation:?} created no id"),
        },
        Err(error) => panic!("{ROW}: setup could not apply {operation:?}: {error}"),
    }
}

/// A document of one `ArgumentPackage` root `p` holding the given Claims, in
/// order, all unnamed: the base of most single-feature pairs.
fn package_with_claims(claims: &[&str]) -> Document {
    let mut doc = Document::new();
    add(&mut doc, None, ElementKind::ArgumentPackage, "p", None);
    for claim in claims {
        add(&mut doc, Some("p"), ElementKind::Claim, claim, None);
    }
    doc
}

/// A document of the given unnamed `ArgumentPackage` roots, in order.
fn roots(names: &[&str]) -> Document {
    let mut doc = Document::new();
    for root in names {
        add(&mut doc, None, ElementKind::ArgumentPackage, root, None);
    }
    doc
}

/// `difference` as `semantic_compare(right, left)` reports it: left and right
/// swapped.
fn swapped(difference: &Difference) -> Difference {
    match difference.clone() {
        Difference::RootOrder { left, right } => Difference::RootOrder {
            left: right,
            right: left,
        },
        Difference::LeftOnly { id, kind } => Difference::RightOnly { id, kind },
        Difference::RightOnly { id, kind } => Difference::LeftOnly { id, kind },
        Difference::Kind { id, left, right } => Difference::Kind {
            id,
            left: right,
            right: left,
        },
        Difference::Parent { id, left, right } => Difference::Parent {
            id,
            left: right,
            right: left,
        },
        Difference::Name { id, left, right } => Difference::Name {
            id,
            left: right,
            right: left,
        },
        Difference::AssertionDeclaration { id, left, right } => Difference::AssertionDeclaration {
            id,
            left: right,
            right: left,
        },
        Difference::ChildOrder { id, left, right } => Difference::ChildOrder {
            id,
            left: right,
            right: left,
        },
        other => panic!("{ROW}: the test's `swapped` does not know {other:?}"),
    }
}

/// Asserts that `left` and `right` are equal both ways: `semantic_compare`
/// finds nothing and `==` holds.
fn assert_equal(what: &str, left: &Document, right: &Document) {
    assert_eq!(
        semantic_compare(left, right),
        [],
        "{ROW}: {what}: semantic_compare(left, right) is not empty"
    );
    assert_eq!(
        semantic_compare(right, left),
        [],
        "{ROW}: {what}: semantic_compare(right, left) is not empty"
    );
    assert!(left == right, "{ROW}: {what}: left == right is false");
    assert!(right == left, "{ROW}: {what}: right == left is false");
}

/// Asserts that `semantic_compare(left, right)` is exactly `expected`, that
/// the swapped comparison is `expected` swapped, as the same set, and that
/// `==` is false both ways.
fn assert_differences(what: &str, left: &Document, right: &Document, expected: &[Difference]) {
    assert_eq!(
        semantic_compare(left, right),
        expected,
        "{ROW}: {what}: semantic_compare(left, right) is not the expected differences"
    );
    let back = semantic_compare(right, left);
    let back_set: HashSet<Difference> = back.iter().cloned().collect();
    let expected_back: HashSet<Difference> = expected.iter().map(swapped).collect();
    assert_eq!(
        (back.len(), back_set),
        (expected.len(), expected_back),
        "{ROW}: {what}: semantic_compare(right, left) is not the differences swapped"
    );
    assert!(left != right, "{ROW}: {what}: left != right is false");
    assert!(right != left, "{ROW}: {what}: right != left is false");
}

/// Asserts that `left` and `right` differ in exactly `expected`, both ways.
fn assert_only(what: &str, left: &Document, right: &Document, expected: Difference) {
    assert_differences(what, left, right, &[expected]);
}

/// A document built with `edit::apply` only: three roots (two
/// `AssuranceCasePackage`s and an `ArgumentPackage`), depth 4, every create
/// operation, generated and caller ids, and names in all nine absent, empty
/// and non-ASCII `(lang, content)` states. Returns it with its Claims' ids.
fn built_with_edit() -> (Document, Vec<ElementId>) {
    let mut doc = Document::new();
    let case = create(
        &mut doc,
        CreateAssuranceCasePackage::root(en("Safety case")),
    );
    let sub_case = create(
        &mut doc,
        CreateAssuranceCasePackage::new(&case, LangString::default()).with_id("sub_case"),
    );
    let arguments = create(
        &mut doc,
        CreateArgumentPackage::new(&sub_case, lang_string(Some(""), Some(""))),
    );
    let inner = create(
        &mut doc,
        CreateArgumentPackage::new(&arguments, en("Inner")).with_id("påstående_paket"),
    );
    let mut claims = Vec::new();
    for (n, lang) in FIELD_STATES.into_iter().enumerate() {
        for content in FIELD_STATES {
            let name = lang_string(lang, content);
            let operation = if n == 1 {
                CreateClaim::new(&inner, name).with_id(format!("claim_given_{}", claims.len()))
            } else {
                CreateClaim::new(&inner, name)
            };
            claims.push(create(&mut doc, operation));
        }
    }
    let second = create(&mut doc, CreateArgumentPackage::root(en("Second root")));
    claims.push(create(&mut doc, CreateClaim::new(&second, en("C"))));
    create(
        &mut doc,
        CreateAssuranceCasePackage::root(LangString::new("Ünïcödé")).with_id("third_root"),
    );
    (doc, claims)
}

/// [`built_with_edit`], with its Claims given each of the six declarations
/// in turn (some twice).
fn built_with_declarations() -> Document {
    let (mut doc, claims) = built_with_edit();
    for (claim, declaration) in claims
        .iter()
        .zip(AssertionDeclaration::ALL.iter().copied().cycle())
    {
        declare(&mut doc, claim, declaration);
    }
    doc
}

/// A pair of documents with every kind of difference, built so that each
/// variant appears in a known place (see
/// `lib_rt_001_several_differences_come_in_the_adr_0020_order`).
fn many_differences() -> (Document, Document) {
    let mut left = Document::new();
    add(
        &mut left,
        None,
        ElementKind::ArgumentPackage,
        "p1",
        Some(en("P1")),
    );
    add(
        &mut left,
        None,
        ElementKind::ArgumentPackage,
        "p2",
        Some(en("P2")),
    );
    add(
        &mut left,
        Some("p1"),
        ElementKind::Claim,
        "k",
        Some(en("K")),
    );
    add(&mut left, Some("k"), ElementKind::Note, "n1", None);
    add(&mut left, Some("k"), ElementKind::Note, "n2", None);
    add(&mut left, Some("p1"), ElementKind::Claim, "gone", None);

    let mut right = Document::new();
    add(
        &mut right,
        None,
        ElementKind::ArgumentPackage,
        "p2",
        Some(en("P2 renamed")),
    );
    add(
        &mut right,
        None,
        ElementKind::ArgumentPackage,
        "p1",
        Some(en("P1")),
    );
    add(
        &mut right,
        None,
        ElementKind::ArgumentPackage,
        "extra",
        None,
    );
    add(
        &mut right,
        Some("p2"),
        ElementKind::AssertedInference,
        "k",
        Some(en("K2")),
    );
    add(&mut right, Some("k"), ElementKind::Note, "n2", None);
    add(&mut right, Some("k"), ElementKind::Note, "n1", None);
    add(&mut right, Some("extra"), ElementKind::Claim, "e1", None);
    declare(&mut right, &id("k"), AssertionDeclaration::Assumed);
    (left, right)
}

/// The differences of [`many_differences`], in the ADR 0020 order.
fn many_differences_expected() -> Vec<Difference> {
    vec![
        // 1. Root order: the common roots p1 and p2, each side's order.
        Difference::RootOrder {
            left: ids(&["p1", "p2"]),
            right: ids(&["p2", "p1"]),
        },
        // 2. Left's pre-order is p1, k, n1, n2, gone, p2. p1, n1 and n2 hold
        //    no difference; k holds one of each feature, in the fixed order.
        Difference::Kind {
            id: id("k"),
            left: ElementKind::Claim,
            right: ElementKind::AssertedInference,
        },
        Difference::Parent {
            id: id("k"),
            left: Some(id("p1")),
            right: Some(id("p2")),
        },
        Difference::Name {
            id: id("k"),
            left: Some(en("K")),
            right: Some(en("K2")),
        },
        Difference::AssertionDeclaration {
            id: id("k"),
            left: Some(AssertionDeclaration::Asserted),
            right: Some(AssertionDeclaration::Assumed),
        },
        Difference::ChildOrder {
            id: id("k"),
            left: ids(&["n1", "n2"]),
            right: ids(&["n2", "n1"]),
        },
        Difference::LeftOnly {
            id: id("gone"),
            kind: ElementKind::Claim,
        },
        Difference::Name {
            id: id("p2"),
            left: Some(en("P2")),
            right: Some(en("P2 renamed")),
        },
        // 3. Right-only, in right's pre-order: p2, k, n2, n1, p1, extra, e1.
        Difference::RightOnly {
            id: id("extra"),
            kind: ElementKind::ArgumentPackage,
        },
        Difference::RightOnly {
            id: id("e1"),
            kind: ElementKind::Claim,
        },
    ]
}

/// Reads `input`, which the test expects to be read.
fn read(what: &str, input: &str) -> Document {
    match xmi::read(input.as_bytes(), &ReadOptions::default()) {
        Ok(output) => output.into_document(),
        Err(error) => panic!("{ROW}: setup: {what} does not read: {error}\n{input}"),
    }
}

// ---------------------------------------------------------------------------
// Item 1: equal documents
// ---------------------------------------------------------------------------

#[test]
fn lib_rt_001_a_document_equals_itself_and_its_clone() {
    let empty = Document::new();
    assert_equal("an empty document and itself", &empty, &empty);
    assert_equal("two empty documents", &empty, &Document::new());

    let doc = built_with_declarations();
    assert_equal("a document and itself", &doc, &doc);
    assert_equal("a document and its clone", &doc, &doc.clone());

    let (left, right) = many_differences();
    assert_equal("a seam-built document and its clone", &left, &left.clone());
    assert_equal(
        "another seam-built document and its clone",
        &right,
        &right.clone(),
    );
}

#[test]
fn lib_rt_001_documents_built_by_the_same_operations_are_equal() {
    let (first, _) = built_with_edit();
    let (second, _) = built_with_edit();
    assert!(
        first.roots().count() >= 2,
        "{ROW}: setup: the built document has fewer than two roots"
    );
    assert_equal(
        "two documents built by the same operations",
        &first,
        &second,
    );
    assert_equal(
        "two documents built by the same operations, with declarations",
        &built_with_declarations(),
        &built_with_declarations(),
    );
}

#[test]
fn lib_rt_001_documents_with_each_declaration_set_the_same_way_are_equal() {
    for &declaration in AssertionDeclaration::ALL {
        let build = || {
            let (mut doc, claims) = built_with_edit();
            for claim in &claims {
                declare(&mut doc, claim, declaration);
            }
            doc
        };
        assert_equal(
            &format!("two documents whose Claims are all {declaration}"),
            &build(),
            &build(),
        );
    }
}

// ---------------------------------------------------------------------------
// Item 2: the generated-id counters are not compared
// ---------------------------------------------------------------------------

#[test]
fn lib_rt_001_generated_and_caller_ids_with_the_same_content_are_equal() {
    let mut generated = Document::new();
    let case = create(&mut generated, CreateAssuranceCasePackage::root(en("Case")));
    let arguments = create(
        &mut generated,
        CreateArgumentPackage::new(&case, en("Arguments")),
    );
    create(&mut generated, CreateClaim::new(&arguments, en("C1")));
    create(&mut generated, CreateClaim::new(&arguments, en("C2")));

    // The same content with caller ids, which advance no counter.
    let mut given = Document::new();
    create(
        &mut given,
        CreateAssuranceCasePackage::root(en("Case")).with_id("assurance_case_package_1"),
    );
    create(
        &mut given,
        CreateArgumentPackage::new("assurance_case_package_1", en("Arguments"))
            .with_id("argument_package_1"),
    );
    create(
        &mut given,
        CreateClaim::new("argument_package_1", en("C1")).with_id("claim_1"),
    );
    create(
        &mut given,
        CreateClaim::new("argument_package_1", en("C2")).with_id("claim_2"),
    );
    assert_equal(
        "generated ids and the same ids given by the caller",
        &generated,
        &given,
    );

    // The same content inserted through the seam, which has no counters.
    let mut inserted = Document::new();
    add(
        &mut inserted,
        None,
        ElementKind::AssuranceCasePackage,
        "assurance_case_package_1",
        Some(en("Case")),
    );
    add(
        &mut inserted,
        Some("assurance_case_package_1"),
        ElementKind::ArgumentPackage,
        "argument_package_1",
        Some(en("Arguments")),
    );
    add(
        &mut inserted,
        Some("argument_package_1"),
        ElementKind::Claim,
        "claim_1",
        Some(en("C1")),
    );
    add(
        &mut inserted,
        Some("argument_package_1"),
        ElementKind::Claim,
        "claim_2",
        Some(en("C2")),
    );
    assert_equal(
        "generated ids and the same content inserted",
        &generated,
        &inserted,
    );
}

// ---------------------------------------------------------------------------
// Item 3: each feature is its own variant
// ---------------------------------------------------------------------------

#[test]
fn lib_rt_001_an_element_on_one_side_only_is_left_only_or_right_only_with_its_kind() {
    let base = package_with_claims(&["c1"]);
    let cases: [(Option<&str>, ElementKind, &str); 4] = [
        (Some("p"), ElementKind::Note, "n1"),
        (Some("p"), ElementKind::ArgumentReasoning, "r1"),
        (Some("p"), ElementKind::Claim, "c2"),
        (None, ElementKind::ArtifactPackage, "artifacts"),
    ];
    for (parent, kind, element) in cases {
        let mut more = base.clone();
        add(&mut more, parent, kind, element, None);
        assert_only(
            &format!("a {kind} only on the left"),
            &more,
            &base,
            Difference::LeftOnly {
                id: id(element),
                kind,
            },
        );
        assert_only(
            &format!("a {kind} only on the right"),
            &base,
            &more,
            Difference::RightOnly {
                id: id(element),
                kind,
            },
        );
    }
}

#[test]
fn lib_rt_001_a_kind_change_is_one_kind_difference() {
    // Both are Assertions with the default declaration, so only the kind differs.
    let claim = package_with_claims(&["x"]);
    let mut inference = Document::new();
    add(
        &mut inference,
        None,
        ElementKind::ArgumentPackage,
        "p",
        None,
    );
    add(
        &mut inference,
        Some("p"),
        ElementKind::AssertedInference,
        "x",
        None,
    );
    assert_only(
        "a Claim and an AssertedInference with one id",
        &claim,
        &inference,
        Difference::Kind {
            id: id("x"),
            left: ElementKind::Claim,
            right: ElementKind::AssertedInference,
        },
    );

    // Two root package kinds, neither an Assertion.
    let mut case = Document::new();
    add(
        &mut case,
        None,
        ElementKind::AssuranceCasePackage,
        "r",
        None,
    );
    let mut arguments = Document::new();
    add(
        &mut arguments,
        None,
        ElementKind::ArgumentPackage,
        "r",
        None,
    );
    assert_only(
        "an AssuranceCasePackage and an ArgumentPackage root with one id",
        &case,
        &arguments,
        Difference::Kind {
            id: id("r"),
            left: ElementKind::AssuranceCasePackage,
            right: ElementKind::ArgumentPackage,
        },
    );
}

#[test]
fn lib_rt_001_an_element_whose_kind_differs_is_still_compared_on_its_other_features() {
    // ADR 0020 C6: a Claim has a declaration, an ArgumentReasoning has none.
    let mut claim = Document::new();
    add(&mut claim, None, ElementKind::ArgumentPackage, "p", None);
    add(
        &mut claim,
        Some("p"),
        ElementKind::Claim,
        "x",
        Some(en("X")),
    );
    let mut reasoning = Document::new();
    add(
        &mut reasoning,
        None,
        ElementKind::ArgumentPackage,
        "p",
        None,
    );
    add(
        &mut reasoning,
        Some("p"),
        ElementKind::ArgumentReasoning,
        "x",
        Some(en("Y")),
    );
    assert_differences(
        "a Claim and an ArgumentReasoning with one id and different names",
        &claim,
        &reasoning,
        &[
            Difference::Kind {
                id: id("x"),
                left: ElementKind::Claim,
                right: ElementKind::ArgumentReasoning,
            },
            Difference::Name {
                id: id("x"),
                left: Some(en("X")),
                right: Some(en("Y")),
            },
            Difference::AssertionDeclaration {
                id: id("x"),
                left: Some(AssertionDeclaration::Asserted),
                right: None,
            },
        ],
    );
}

#[test]
fn lib_rt_001_a_different_parent_is_one_parent_difference() {
    let mut left = roots(&["p1", "p2"]);
    add(&mut left, Some("p1"), ElementKind::Claim, "c", None);
    let mut right = roots(&["p1", "p2"]);
    add(&mut right, Some("p2"), ElementKind::Claim, "c", None);
    assert_only(
        "a Claim in p1 and in p2",
        &left,
        &right,
        Difference::Parent {
            id: id("c"),
            left: Some(id("p1")),
            right: Some(id("p2")),
        },
    );
}

#[test]
fn lib_rt_001_a_root_and_a_contained_element_differ_in_parent() {
    let mut contained = roots(&["p"]);
    add(
        &mut contained,
        Some("p"),
        ElementKind::ArgumentPackage,
        "q",
        None,
    );
    let root = roots(&["p", "q"]);
    assert_only(
        "q contained in p on the left and a root on the right",
        &contained,
        &root,
        Difference::Parent {
            id: id("q"),
            left: Some(id("p")),
            right: None,
        },
    );
    assert_only(
        "q a root on the left and contained in p on the right",
        &root,
        &contained,
        Difference::Parent {
            id: id("q"),
            left: None,
            right: Some(id("p")),
        },
    );
}

#[test]
fn lib_rt_001_a_different_declaration_is_one_assertion_declaration_difference() {
    for &on_left in AssertionDeclaration::ALL {
        for &on_right in AssertionDeclaration::ALL {
            let mut left = package_with_claims(&["c1", "c2"]);
            declare(&mut left, &id("c2"), on_left);
            let mut right = package_with_claims(&["c1", "c2"]);
            declare(&mut right, &id("c2"), on_right);
            let what = format!("a Claim {on_left} on the left and {on_right} on the right");
            if on_left == on_right {
                assert_equal(&what, &left, &right);
            } else {
                assert_only(
                    &what,
                    &left,
                    &right,
                    Difference::AssertionDeclaration {
                        id: id("c2"),
                        left: Some(on_left),
                        right: Some(on_right),
                    },
                );
            }
        }
    }
}

#[test]
fn lib_rt_001_a_set_declaration_differs_from_the_default() {
    let default = package_with_claims(&["c1"]);
    let mut set = package_with_claims(&["c1"]);
    declare(&mut set, &id("c1"), AssertionDeclaration::NeedsSupport);
    assert_only(
        "a Claim with the default declaration and one set to needsSupport",
        &default,
        &set,
        Difference::AssertionDeclaration {
            id: id("c1"),
            left: Some(AssertionDeclaration::Asserted),
            right: Some(AssertionDeclaration::NeedsSupport),
        },
    );
    // Setting the default explicitly is the same value.
    let mut asserted = package_with_claims(&["c1"]);
    declare(&mut asserted, &id("c1"), AssertionDeclaration::Asserted);
    assert_equal(
        "a default and an explicitly asserted Claim",
        &default,
        &asserted,
    );
}

// ---------------------------------------------------------------------------
// Item 4: names, compared exactly
// ---------------------------------------------------------------------------

/// A document of an `ArgumentPackage` root `p` holding one Claim `c` named
/// `name` (missing when `None`).
fn claim_named(name: Option<LangString>) -> Document {
    let mut doc = Document::new();
    add(
        &mut doc,
        None,
        ElementKind::ArgumentPackage,
        "p",
        Some(en("P")),
    );
    add(&mut doc, Some("p"), ElementKind::Claim, "c", name);
    doc
}

/// Asserts that `left` and `right` as names of `c` give exactly one `Name`
/// difference.
fn assert_name_difference(what: &str, left: Option<LangString>, right: Option<LangString>) {
    assert_only(
        what,
        &claim_named(left.clone()),
        &claim_named(right.clone()),
        Difference::Name {
            id: id("c"),
            left,
            right,
        },
    );
}

#[test]
fn lib_rt_001_a_missing_name_differs_from_an_empty_one() {
    assert_name_difference(
        "a missing name and `LangString::default()` (`<name/>`)",
        None,
        Some(LangString::default()),
    );
    assert_name_difference(
        "a missing name and `content=\"\"`",
        None,
        Some(LangString::new("")),
    );
    assert_name_difference(
        "a missing name and `lang=\"\" content=\"\"`",
        None,
        Some(lang_string(Some(""), Some(""))),
    );
}

#[test]
fn lib_rt_001_an_absent_lang_differs_from_an_empty_one() {
    assert_name_difference(
        "`lang` absent and `lang=\"\"`",
        Some(LangString::new("C")),
        Some(LangString::new("C").with_lang("")),
    );
    assert_name_difference(
        "`content` absent and `content=\"\"`",
        Some(lang_string(Some("en"), None)),
        Some(lang_string(Some("en"), Some(""))),
    );
}

#[test]
fn lib_rt_001_different_content_or_lang_is_one_name_difference() {
    assert_name_difference("different content", Some(en("C1")), Some(en("C2")));
    assert_name_difference(
        "different lang",
        Some(LangString::new("C").with_lang("en")),
        Some(LangString::new("C").with_lang("sv")),
    );
    assert_name_difference(
        "content differing only in a non-ASCII character",
        Some(en("Säkerhet")),
        Some(en("Sakerhet")),
    );
}

#[test]
fn lib_rt_001_every_pair_of_name_states_is_one_name_difference_or_none() {
    // A missing name and the nine (lang, content) states.
    let mut states = vec![None];
    for lang in FIELD_STATES {
        for content in FIELD_STATES {
            states.push(Some(lang_string(lang, content)));
        }
    }
    for left in &states {
        for right in &states {
            let what = format!("the names {left:?} and {right:?}");
            if left == right {
                assert_equal(
                    &what,
                    &claim_named(left.clone()),
                    &claim_named(right.clone()),
                );
            } else {
                assert_name_difference(&what, left.clone(), right.clone());
            }
        }
    }
}

// ---------------------------------------------------------------------------
// Item 5: order
// ---------------------------------------------------------------------------

#[test]
fn lib_rt_001_swapping_two_siblings_of_one_role_is_one_child_order() {
    assert_only(
        "Claims c1, c2 and c2, c1 in p",
        &package_with_claims(&["c1", "c2"]),
        &package_with_claims(&["c2", "c1"]),
        Difference::ChildOrder {
            id: id("p"),
            left: ids(&["c1", "c2"]),
            right: ids(&["c2", "c1"]),
        },
    );
    // Only the swapped pair moves; the order is that of every common child.
    assert_only(
        "Claims c1, c2, c3 and c1, c3, c2 in p",
        &package_with_claims(&["c1", "c2", "c3"]),
        &package_with_claims(&["c1", "c3", "c2"]),
        Difference::ChildOrder {
            id: id("p"),
            left: ids(&["c1", "c2", "c3"]),
            right: ids(&["c1", "c3", "c2"]),
        },
    );
}

#[test]
fn lib_rt_001_swapping_two_siblings_of_different_roles_is_one_child_order() {
    // `note` (a ModelElement role) and `argumentElement` (ArgumentPackage's).
    let in_order = |first: (ElementKind, &str), second: (ElementKind, &str)| {
        let mut doc = roots(&["p"]);
        add(&mut doc, Some("p"), first.0, first.1, None);
        add(&mut doc, Some("p"), second.0, second.1, None);
        doc
    };
    let note = (ElementKind::Note, "n");
    let claim = (ElementKind::Claim, "c");
    assert_only(
        "a Note and a Claim in p, swapped",
        &in_order(note, claim),
        &in_order(claim, note),
        Difference::ChildOrder {
            id: id("p"),
            left: ids(&["n", "c"]),
            right: ids(&["c", "n"]),
        },
    );

    // `argumentPackage` and `artifactPackage` of an AssuranceCasePackage.
    let case = |first: (ElementKind, &str), second: (ElementKind, &str)| {
        let mut doc = Document::new();
        add(
            &mut doc,
            None,
            ElementKind::AssuranceCasePackage,
            "case",
            None,
        );
        add(&mut doc, Some("case"), first.0, first.1, None);
        add(&mut doc, Some("case"), second.0, second.1, None);
        doc
    };
    let arguments = (ElementKind::ArgumentPackage, "arguments");
    let artifacts = (ElementKind::ArtifactPackage, "artifacts");
    assert_only(
        "an ArgumentPackage and an ArtifactPackage in case, swapped",
        &case(arguments, artifacts),
        &case(artifacts, arguments),
        Difference::ChildOrder {
            id: id("case"),
            left: ids(&["arguments", "artifacts"]),
            right: ids(&["artifacts", "arguments"]),
        },
    );
}

#[test]
fn lib_rt_001_swapping_two_roots_is_one_root_order() {
    assert_only(
        "roots a, b and b, a",
        &roots(&["a", "b"]),
        &roots(&["b", "a"]),
        Difference::RootOrder {
            left: ids(&["a", "b"]),
            right: ids(&["b", "a"]),
        },
    );
    assert_only(
        "roots a, b, c and c, b, a",
        &roots(&["a", "b", "c"]),
        &roots(&["c", "b", "a"]),
        Difference::RootOrder {
            left: ids(&["a", "b", "c"]),
            right: ids(&["c", "b", "a"]),
        },
    );
}

#[test]
fn lib_rt_001_adding_one_child_is_one_right_only_and_no_order_difference() {
    let base = package_with_claims(&["c1", "c3"]);
    for (position, with) in [
        ("at the end", package_with_claims(&["c1", "c3", "c2"])),
        ("in the middle", package_with_claims(&["c1", "c2", "c3"])),
        ("at the front", package_with_claims(&["c2", "c1", "c3"])),
    ] {
        assert_only(
            &format!("a Claim added {position}"),
            &base,
            &with,
            Difference::RightOnly {
                id: id("c2"),
                kind: ElementKind::Claim,
            },
        );
    }
}

#[test]
fn lib_rt_001_adding_or_removing_a_root_is_no_order_difference() {
    assert_only(
        "a root added at the front",
        &roots(&["a", "b"]),
        &roots(&["n", "a", "b"]),
        Difference::RightOnly {
            id: id("n"),
            kind: ElementKind::ArgumentPackage,
        },
    );
    assert_only(
        "a root removed from the middle",
        &roots(&["a", "b", "c"]),
        &roots(&["a", "c"]),
        Difference::LeftOnly {
            id: id("b"),
            kind: ElementKind::ArgumentPackage,
        },
    );
}

// ---------------------------------------------------------------------------
// Item 6: one-side subtrees and moved elements
// ---------------------------------------------------------------------------

#[test]
fn lib_rt_001_a_package_only_on_the_right_gives_every_element_in_pre_order() {
    let left = roots(&["p"]);
    let mut right = roots(&["p", "q"]);
    add(&mut right, Some("q"), ElementKind::Claim, "q1", None);
    add(&mut right, Some("q"), ElementKind::Claim, "q2", None);
    assert_differences(
        "a root package holding two Claims, only on the right",
        &left,
        &right,
        &[
            Difference::RightOnly {
                id: id("q"),
                kind: ElementKind::ArgumentPackage,
            },
            Difference::RightOnly {
                id: id("q1"),
                kind: ElementKind::Claim,
            },
            Difference::RightOnly {
                id: id("q2"),
                kind: ElementKind::Claim,
            },
        ],
    );
}

#[test]
fn lib_rt_001_a_nested_package_only_on_the_left_gives_every_element_in_pre_order() {
    let mut left = package_with_claims(&["c1"]);
    add(
        &mut left,
        Some("p"),
        ElementKind::ArgumentPackage,
        "q",
        None,
    );
    add(&mut left, Some("q"), ElementKind::Claim, "q1", None);
    add(&mut left, Some("q1"), ElementKind::Note, "q1_note", None);
    add(&mut left, Some("q"), ElementKind::Claim, "q2", None);
    add(&mut left, Some("p"), ElementKind::Claim, "c2", None);
    let right = package_with_claims(&["c1", "c2"]);
    assert_differences(
        "a nested package holding two Claims and a Note, only on the left",
        &left,
        &right,
        &[
            Difference::LeftOnly {
                id: id("q"),
                kind: ElementKind::ArgumentPackage,
            },
            Difference::LeftOnly {
                id: id("q1"),
                kind: ElementKind::Claim,
            },
            Difference::LeftOnly {
                id: id("q1_note"),
                kind: ElementKind::Note,
            },
            Difference::LeftOnly {
                id: id("q2"),
                kind: ElementKind::Claim,
            },
        ],
    );
}

#[test]
fn lib_rt_001_a_moved_subtree_is_one_parent_difference() {
    let build = |parent: &str| {
        let mut doc = roots(&["p1", "p2"]);
        add(
            &mut doc,
            Some(parent),
            ElementKind::ArgumentPackage,
            "moved",
            None,
        );
        add(&mut doc, Some("moved"), ElementKind::Claim, "m1", None);
        add(&mut doc, Some("moved"), ElementKind::Claim, "m2", None);
        add(&mut doc, Some("p1"), ElementKind::Claim, "stays", None);
        doc
    };
    let differences = semantic_compare(&build("p1"), &build("p2"));
    assert!(
        !differences.iter().any(|difference| matches!(
            difference,
            Difference::LeftOnly { .. } | Difference::RightOnly { .. }
        )),
        "{ROW}: a moved subtree is reported as left-only or right-only: {differences:?}"
    );
    assert_only(
        "a package holding two Claims, moved from p1 to p2",
        &build("p1"),
        &build("p2"),
        Difference::Parent {
            id: id("moved"),
            left: Some(id("p1")),
            right: Some(id("p2")),
        },
    );
}

// ---------------------------------------------------------------------------
// Item 7: output order and determinism
// ---------------------------------------------------------------------------

#[test]
fn lib_rt_001_several_differences_come_in_the_adr_0020_order() {
    let (left, right) = many_differences();
    assert_differences(
        "a pair with every kind of difference",
        &left,
        &right,
        &many_differences_expected(),
    );
}

#[test]
fn lib_rt_001_the_result_is_the_same_on_every_run() {
    let (left, right) = many_differences();
    let first = semantic_compare(&left, &right);
    for run in 0..20 {
        assert_eq!(
            semantic_compare(&left, &right),
            first,
            "{ROW}: run {run} gives a different result"
        );
        assert_eq!(
            semantic_compare(&left.clone(), &right.clone()),
            first,
            "{ROW}: run {run} on clones gives a different result"
        );
    }
    // Rebuilt from scratch, the same pair gives the same result.
    let (left, right) = many_differences();
    assert_eq!(
        semantic_compare(&left, &right),
        first,
        "{ROW}: the rebuilt pair gives a different result"
    );
}

// ---------------------------------------------------------------------------
// Item 8: symmetry
// ---------------------------------------------------------------------------

#[test]
fn lib_rt_001_comparison_is_symmetric() {
    let (left, right) = many_differences();
    let forward = semantic_compare(&left, &right);
    let backward = semantic_compare(&right, &left);
    let swapped_forward: HashSet<Difference> = forward.iter().map(swapped).collect();
    let backward_set: HashSet<Difference> = backward.iter().cloned().collect();
    assert_eq!(
        (backward.len(), backward_set),
        (forward.len(), swapped_forward),
        "{ROW}: semantic_compare(b, a) is not semantic_compare(a, b) swapped"
    );

    let (built, _) = built_with_edit();
    let documents = [
        Document::new(),
        left,
        right,
        built,
        built_with_declarations(),
    ];
    for (i, one) in documents.iter().enumerate() {
        for (j, other) in documents.iter().enumerate() {
            assert_eq!(
                one == other,
                other == one,
                "{ROW}: documents {i} and {j}: == is not symmetric"
            );
            assert_eq!(
                one == other,
                semantic_compare(one, other).is_empty(),
                "{ROW}: documents {i} and {j}: == is not \"semantic_compare is empty\""
            );
        }
    }
}

// ---------------------------------------------------------------------------
// Item 9: element_id and Display
// ---------------------------------------------------------------------------

/// One `Difference` of each variant, about the element `of`, with the
/// feature name its `Display` must contain.
fn one_of_each(of: &ElementId) -> Vec<(Difference, &'static str)> {
    vec![
        (
            Difference::LeftOnly {
                id: of.clone(),
                kind: ElementKind::Note,
            },
            "Note",
        ),
        (
            Difference::RightOnly {
                id: of.clone(),
                kind: ElementKind::ArgumentReasoning,
            },
            "ArgumentReasoning",
        ),
        (
            Difference::Kind {
                id: of.clone(),
                left: ElementKind::Claim,
                right: ElementKind::AssertedInference,
            },
            "kind",
        ),
        (
            Difference::Parent {
                id: of.clone(),
                left: Some(id("p1")),
                right: None,
            },
            "parent",
        ),
        (
            Difference::Name {
                id: of.clone(),
                left: None,
                right: Some(LangString::default()),
            },
            "name",
        ),
        (
            Difference::AssertionDeclaration {
                id: of.clone(),
                left: Some(AssertionDeclaration::Asserted),
                right: None,
            },
            "assertionDeclaration",
        ),
        (
            Difference::ChildOrder {
                id: of.clone(),
                left: ids(&["c1", "c2"]),
                right: ids(&["c2", "c1"]),
            },
            "order",
        ),
    ]
}

#[test]
fn lib_rt_001_element_id_is_the_element_the_parent_or_none() {
    let of = id("c1");
    for (difference, _) in one_of_each(&of) {
        assert_eq!(
            difference.element_id(),
            Some(&of),
            "{ROW}: element_id of {difference:?}"
        );
    }
    let (left, right) = many_differences();
    for difference in semantic_compare(&left, &right) {
        let expected = match &difference {
            Difference::RootOrder { .. } => None,
            Difference::LeftOnly { id, .. }
            | Difference::RightOnly { id, .. }
            | Difference::Kind { id, .. }
            | Difference::Parent { id, .. }
            | Difference::Name { id, .. }
            | Difference::AssertionDeclaration { id, .. }
            | Difference::ChildOrder { id, .. } => Some(id.clone()),
            other => panic!("{ROW}: the test does not know {other:?}"),
        };
        assert_eq!(
            difference.element_id().cloned(),
            expected,
            "{ROW}: element_id of {difference:?}"
        );
    }
    let root_order = Difference::RootOrder {
        left: ids(&["a", "b"]),
        right: ids(&["b", "a"]),
    };
    assert_eq!(
        root_order.element_id(),
        None,
        "{ROW}: element_id of RootOrder"
    );
    // For ChildOrder it is the parent, whose children are reordered.
    let child_order = semantic_compare(
        &package_with_claims(&["c1", "c2"]),
        &package_with_claims(&["c2", "c1"]),
    );
    let parents: Vec<Option<&ElementId>> = child_order.iter().map(Difference::element_id).collect();
    assert_eq!(parents, [Some(&id("p"))], "{ROW}: element_id of ChildOrder");
}

#[test]
fn lib_rt_001_display_is_one_line_with_the_quoted_id_and_the_feature() {
    for element in ["c1", "a\"b\nc\rd\te", "påstående_1"] {
        let of = id(element);
        let quoted = format!("{element:?}");
        for (difference, feature) in one_of_each(&of) {
            let text = difference.to_string();
            assert!(
                !text.contains('\n') && !text.contains('\r') && !text.is_empty(),
                "{ROW}: the Display of {difference:?} is not one line: {text:?}"
            );
            assert!(
                text.contains(&quoted),
                "{ROW}: the Display of {difference:?} does not contain the quoted id {quoted}: {text:?}"
            );
            assert!(
                text.contains(feature),
                "{ROW}: the Display of {difference:?} does not name the feature {feature:?}: {text:?}"
            );
        }
    }
    let root_order = Difference::RootOrder {
        left: ids(&["a", "b"]),
        right: ids(&["b", "a"]),
    }
    .to_string();
    assert!(
        !root_order.contains('\n') && root_order.contains("root") && root_order.contains("order"),
        "{ROW}: the Display of RootOrder is not one line naming the root order: {root_order:?}"
    );
    let left_only = Difference::LeftOnly {
        id: id("c1"),
        kind: ElementKind::Claim,
    };
    let right_only = Difference::RightOnly {
        id: id("c1"),
        kind: ElementKind::Claim,
    };
    assert_ne!(
        left_only.to_string(),
        right_only.to_string(),
        "{ROW}: LeftOnly and RightOnly read the same"
    );
}

/// Asserts that `text` holds `list` as one run of quoted ids, in order, each
/// pair separated by `, `, with no separator before the first id or after the
/// last; returns where the run starts.
fn assert_list_shown(what: &str, text: &str, list: &[ElementId]) -> usize {
    let run = list
        .iter()
        .map(|element| format!("{:?}", element.as_str()))
        .collect::<Vec<_>>()
        .join(", ");
    let Some(start) = text.find(&run) else {
        panic!("{ROW}: the Display of {what} does not show the list {run} in order: {text:?}");
    };
    let before = text[..start].trim_end();
    let after = text[start + run.len()..].trim_start();
    assert!(
        !before.ends_with(',') && !after.starts_with(','),
        "{ROW}: the Display of {what} has a separator before or after the list {run}: {text:?}"
    );
    start
}

#[test]
fn lib_rt_001_display_shows_both_values() {
    // Parent: both parent ids, and a root reads differently from a contained side.
    let moved = Difference::Parent {
        id: id("moved"),
        left: Some(id("p1")),
        right: Some(id("p2")),
    };
    let text = moved.to_string();
    for parent in ["\"p1\"", "\"p2\""] {
        assert!(
            text.contains(parent),
            "{ROW}: the Display of {moved:?} does not show the parent {parent}: {text:?}"
        );
    }
    let from_root = Difference::Parent {
        id: id("moved"),
        left: None,
        right: Some(id("p2")),
    };
    assert_ne!(
        from_root.to_string(),
        moved.to_string(),
        "{ROW}: a Parent difference reads the same for a root and for a contained side"
    );
    assert!(
        !from_root.to_string().contains("\"p1\""),
        "{ROW}: the Display of {from_root:?} shows a parent the root does not have: {from_root}"
    );

    // AssertionDeclaration: both literals, and `None` reads differently from `Some`.
    let declared = Difference::AssertionDeclaration {
        id: id("c1"),
        left: Some(AssertionDeclaration::NeedsSupport),
        right: Some(AssertionDeclaration::Assumed),
    };
    let text = declared.to_string();
    for literal in ["needsSupport", "assumed"] {
        assert!(
            text.contains(literal),
            "{ROW}: the Display of {declared:?} does not show the literal {literal}: {text:?}"
        );
    }
    let undeclared = Difference::AssertionDeclaration {
        id: id("c1"),
        left: None,
        right: Some(AssertionDeclaration::Assumed),
    };
    assert_ne!(
        undeclared.to_string(),
        declared.to_string(),
        "{ROW}: an AssertionDeclaration difference reads the same for None and for Some"
    );
    assert!(
        !undeclared.to_string().contains("needsSupport"),
        "{ROW}: the Display of {undeclared:?} shows a literal the side does not have: {undeclared}"
    );

    // ChildOrder and RootOrder: every id of both lists, in order, separated,
    // and the left list before the right one.
    let orders = [
        Difference::RootOrder {
            left: ids(&["a", "b", "c"]),
            right: ids(&["c", "b", "a"]),
        },
        Difference::ChildOrder {
            id: id("parent_x"),
            left: ids(&["a", "b", "c"]),
            right: ids(&["b", "c", "a"]),
        },
        Difference::ChildOrder {
            id: id("parent_x"),
            left: ids(&["a", "b"]),
            right: ids(&["b", "a"]),
        },
    ];
    for order in orders {
        let (left, right) = match &order {
            Difference::RootOrder { left, right } | Difference::ChildOrder { left, right, .. } => {
                (left.clone(), right.clone())
            }
            other => panic!("{ROW}: the test does not expect {other:?}"),
        };
        let what = format!("{order:?}");
        let text = order.to_string();
        let left_at = assert_list_shown(&what, &text, &left);
        let right_at = assert_list_shown(&what, &text, &right);
        assert!(
            left_at < right_at,
            "{ROW}: the Display of {what} does not show the left list before the right: {text:?}"
        );
        assert!(
            !text.contains("[, ") && !text.contains(", ]") && !text.contains("\"\""),
            "{ROW}: the Display of {what} has a misplaced or missing separator: {text:?}"
        );
    }
}

#[test]
fn lib_rt_001_display_tells_absent_and_empty_names_apart() {
    let with_left = |left: Option<LangString>| {
        Difference::Name {
            id: id("c1"),
            left,
            right: Some(en("C1")),
        }
        .to_string()
    };
    let texts = [
        ("a missing name", with_left(None)),
        (
            "`LangString::default()`",
            with_left(Some(LangString::default())),
        ),
        ("`content=\"\"`", with_left(Some(LangString::new("")))),
        (
            "`lang=\"\" content=\"\"`",
            with_left(Some(lang_string(Some(""), Some("")))),
        ),
        ("`content=\"x\"`", with_left(Some(LangString::new("x")))),
        (
            "`lang=\"\" content=\"x\"`",
            with_left(Some(LangString::new("x").with_lang(""))),
        ),
    ];
    for (i, (what, text)) in texts.iter().enumerate() {
        for (other, other_text) in &texts[i + 1..] {
            assert_ne!(
                text, other_text,
                "{ROW}: a Name difference reads the same for {what} and {other}"
            );
        }
    }
}

// ---------------------------------------------------------------------------
// Item 10: MultiLangString equality is order-sensitive
// ---------------------------------------------------------------------------

#[test]
fn lib_rt_001_multi_lang_string_equality_is_order_sensitive() {
    let english = en("Safe");
    let swedish = LangString::new("Säker").with_lang("sv");
    let forward: MultiLangString = [english.clone(), swedish.clone()].into_iter().collect();
    let backward: MultiLangString = [swedish.clone(), english.clone()].into_iter().collect();
    assert_ne!(forward, backward, "{ROW}: [en, sv] equals [sv, en]");
    assert_eq!(
        forward,
        [english.clone(), swedish.clone()]
            .into_iter()
            .collect::<MultiLangString>(),
        "{ROW}: [en, sv] does not equal itself"
    );
    // Duplicates count too: [en, en, sv] is not [en, sv, sv].
    let doubled_en: MultiLangString = [english.clone(), english.clone(), swedish.clone()]
        .into_iter()
        .collect();
    let doubled_sv: MultiLangString = [english, swedish.clone(), swedish].into_iter().collect();
    assert_ne!(
        doubled_en, doubled_sv,
        "{ROW}: [en, en, sv] equals [en, sv, sv]"
    );
}

// ---------------------------------------------------------------------------
// The requirement's remaining statements
// ---------------------------------------------------------------------------

/// Compiles only if `T` has the derives the design names, and is shareable.
fn assert_difference_traits<T: Clone + Debug + Display + Eq + Hash + Send + Sync + 'static>() {}

/// Compiles only if `T: Eq`.
fn assert_eq_trait<T: Eq>() {}

#[test]
fn lib_rt_001_difference_and_document_have_the_designed_traits() {
    assert_difference_traits::<Difference>();
    assert_eq_trait::<Document>();
}

#[test]
fn lib_rt_001_formatting_attribute_order_and_prefixes_are_not_compared() {
    let pinned = read(
        "the pinned form",
        concat!(
            "<?xml version=\"1.0\" encoding=\"UTF-8\"?>\n",
            "<sacm:ArgumentPackage xmlns:sacm=\"http://www.omg.org/spec/SACM/20220301\"",
            " xmlns:xmi=\"http://www.omg.org/spec/XMI/20131001\" xmi:version=\"2.0\" xmi:id=\"p\">\n",
            "  <name lang=\"en\" content=\"P\" />\n",
            "  <argumentElement xmi:type=\"sacm:Claim\" xmi:id=\"c1\" assertionDeclaration=\"assumed\" />\n",
            "</sacm:ArgumentPackage>\n",
        ),
    );
    let other = read(
        "another form",
        concat!(
            "<s:ArgumentPackage xmi:id='p' xmlns:x=\"http://www.omg.org/spec/XMI/20131001\"",
            " xmlns:s=\"http://www.omg.org/spec/SACM/20220301\" xmlns:xmi=\"http://www.omg.org/spec/XMI/20131001\">",
            "<name content='P'   lang='en'/><argumentElement assertionDeclaration='assumed'",
            " x:id='c1' x:type='s:Claim'></argumentElement></s:ArgumentPackage>",
        ),
    );
    assert_equal(
        "one document in two forms, with different formatting, attribute order and prefixes",
        &pinned,
        &other,
    );
}

#[test]
fn lib_rt_001_deep_documents_compare_without_panicking() {
    const DEPTH: usize = 20_000;
    let mut doc = Document::new();
    add(&mut doc, None, ElementKind::ArgumentPackage, "p0", None);
    for level in 1..DEPTH {
        let parent = format!("p{}", level - 1);
        add(
            &mut doc,
            Some(&parent),
            ElementKind::ArgumentPackage,
            &format!("p{level}"),
            None,
        );
    }
    assert_equal(
        "a document 20 000 packages deep and its clone",
        &doc,
        &doc.clone(),
    );

    let mut renamed = doc.clone();
    let leaf = format!("p{DEPTH}");
    add(
        &mut renamed,
        Some(&format!("p{}", DEPTH - 1)),
        ElementKind::Claim,
        &leaf,
        None,
    );
    assert_only(
        "a deep document with one more Claim at the bottom",
        &doc,
        &renamed,
        Difference::RightOnly {
            id: id(&leaf),
            kind: ElementKind::Claim,
        },
    );
}
