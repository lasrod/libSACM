//! Tests for the create operations, the first public editing API
//! (LIB-EDIT-001; ADR 0019, ADR 0013, ADR 0014; XMI 2.5.1 §7.6.1).
//!
//! `edit::apply` takes `CreateAssuranceCasePackage`, `CreateArgumentPackage`
//! or `CreateClaim`, checks it completely, and either creates the element or
//! refuses with an `EditError`: `ParentNotFound`, `InvalidParent`,
//! `InvalidId` or `DuplicateId`, in that order, leaving the document
//! unchanged.
//!
//! The expected parent legality is this file's own table, written from
//! `docs/sacm/metamodel-inventory.md` (the containment roles of §9.2, §11.4
//! and §12.2) and ADR 0014, not read from the crate. Parents the edit API
//! cannot create yet are built with the `test-support` `Document::insert`
//! under an ArtifactPackage root, which admits every kind. Documents "read
//! from a file" are read through `xmi::read` from inline XMI.
//!
//! "The next generated id" of a document is observed on a clone: create one
//! element of each kind there with a generated id and see which ids come out.
//!
//! Not checked here:
//! - `#[non_exhaustive]` on `Operation`, `EditError` and its variants: a test
//!   cannot show that an exhaustive match or a struct literal fails to
//!   compile. The verifier checks the attributes in the source and rustdoc.
//! - That a Claim cannot be created as a root: `CreateClaim` has no `root`,
//!   so the misuse does not compile.

use std::error::Error;
use std::path::Path;

use sacm::diagnostic::Code;
use sacm::edit::{
    self, CreateArgumentPackage, CreateAssuranceCasePackage, CreateClaim, EditError, Operation,
};
use sacm::model::{Document, ElementId, ElementKind, LangString, SacmElement};
use sacm::validation;
use sacm::xmi::{self, ReadOptions, WriteOptions};

/// The row every test here verifies.
const ROW: &str = "LIB-EDIT-001";

/// The opening of an `xmi:XMI` wrapper that `xmi::read` reads.
const WRAPPER_OPEN: &str = concat!(
    "<?xml version=\"1.0\" encoding=\"UTF-8\"?>\n",
    "<xmi:XMI xmlns:xmi=\"http://www.omg.org/spec/XMI/20131001\"",
    " xmlns:sacm=\"http://www.omg.org/spec/SACM/20220301\">\n",
);

/// The id of the ArtifactPackage root that test parents of any kind are
/// placed under.
const SCAFFOLD: &str = "scaffold";

// ---------------------------------------------------------------------------
// The expected parent legality, from the metamodel inventory and ADR 0014
// ---------------------------------------------------------------------------

/// AssuranceCasePackage, its interface and its binding (§9.2-§9.4): their
/// `assuranceCasePackage` and `argumentPackage` roles.
const ASSURANCE_CASE_PACKAGES: [ElementKind; 3] = [
    ElementKind::AssuranceCasePackage,
    ElementKind::AssuranceCasePackageInterface,
    ElementKind::AssuranceCasePackageBinding,
];

/// ArgumentPackage, its interface and its binding (§11.4-§11.6): their
/// `argumentElement` role, typed `ArgumentationElement`, which an
/// ArgumentPackage (of any of the three kinds) and a Claim are.
const ARGUMENT_PACKAGES: [ElementKind; 3] = [
    ElementKind::ArgumentPackage,
    ElementKind::ArgumentPackageInterface,
    ElementKind::ArgumentPackageBinding,
];

/// ArtifactPackage, its interface and its binding (§12.2-§12.4): their
/// `artifactElement` role admits every `ArtifactElement`, so every
/// `ModelElement` kind (ADR 0014).
const ARTIFACT_PACKAGES: [ElementKind; 3] = [
    ElementKind::ArtifactPackage,
    ElementKind::ArtifactPackageInterface,
    ElementKind::ArtifactPackageBinding,
];

/// The parent kinds that may contain an element of `child`, one of the three
/// kinds the edit API creates.
fn legal_parents(child: ElementKind) -> Vec<ElementKind> {
    let families: &[[ElementKind; 3]] = match child {
        // §9.2 `assuranceCasePackage`; §12.2 `artifactElement`.
        ElementKind::AssuranceCasePackage => &[ASSURANCE_CASE_PACKAGES, ARTIFACT_PACKAGES],
        // §9.2 `argumentPackage`; §11.4 `argumentElement`; §12.2.
        ElementKind::ArgumentPackage => &[
            ASSURANCE_CASE_PACKAGES,
            ARGUMENT_PACKAGES,
            ARTIFACT_PACKAGES,
        ],
        // §11.4 `argumentElement`; §12.2.
        ElementKind::Claim => &[ARGUMENT_PACKAGES, ARTIFACT_PACKAGES],
        other => panic!("{ROW}: the edit API does not create a {other}"),
    };
    families.iter().flatten().copied().collect()
}

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

/// A non-empty name in English.
fn named(content: &str) -> LangString {
    LangString::new(content).with_lang("en")
}

/// A name with exactly the given `lang` and `content`, each absent when `None`.
fn lang_string(lang: Option<&str>, content: Option<&str>) -> LangString {
    let mut name = LangString::default();
    name.set_lang(lang.map(str::to_owned));
    name.set_content(content.map(str::to_owned));
    name
}

fn id(value: &str) -> ElementId {
    ElementId::from(value)
}

/// Applies an operation the test expects to succeed, checks that it created
/// exactly one element, and returns that element's id.
fn create(doc: &mut Document, operation: impl Into<Operation>) -> ElementId {
    match edit::apply(doc, operation) {
        Ok(mutation) => {
            let created = mutation.created_ids();
            let [only] = created else {
                panic!("{ROW}: created_ids() is {created:?}, not the one new element");
            };
            only.clone()
        }
        Err(error) => panic!("{ROW}: the operation was refused: {error} ({error:?})"),
    }
}

/// Applies an operation the test expects to be refused, and returns why.
fn refuse(doc: &mut Document, operation: impl Into<Operation>) -> EditError {
    match edit::apply(doc, operation) {
        Ok(mutation) => panic!(
            "{ROW}: the operation was accepted, creating {:?}",
            mutation.created_ids()
        ),
        Err(error) => error,
    }
}

/// An `EditError`'s variant and fields, in a form the tests can compare.
#[derive(Debug, PartialEq)]
enum Refusal {
    ParentNotFound {
        parent: ElementId,
    },
    InvalidParent {
        parent: ElementId,
        parent_kind: ElementKind,
        kind: ElementKind,
    },
    InvalidId {
        id: ElementId,
    },
    DuplicateId {
        id: ElementId,
    },
}

fn refusal_of(error: &EditError) -> Refusal {
    match error {
        EditError::ParentNotFound { parent, .. } => Refusal::ParentNotFound {
            parent: parent.clone(),
        },
        EditError::InvalidParent {
            parent,
            parent_kind,
            kind,
            ..
        } => Refusal::InvalidParent {
            parent: parent.clone(),
            parent_kind: *parent_kind,
            kind: *kind,
        },
        EditError::InvalidId { id, .. } => Refusal::InvalidId { id: id.clone() },
        EditError::DuplicateId { id, .. } => Refusal::DuplicateId { id: id.clone() },
        other => panic!("{ROW}: an EditError this row does not define: {other:?}"),
    }
}

fn parent_not_found(parent: &str) -> Refusal {
    Refusal::ParentNotFound { parent: id(parent) }
}

fn invalid_parent(parent: &str, parent_kind: ElementKind, kind: ElementKind) -> Refusal {
    Refusal::InvalidParent {
        parent: id(parent),
        parent_kind,
        kind,
    }
}

fn invalid_id(value: &str) -> Refusal {
    Refusal::InvalidId { id: id(value) }
}

fn duplicate_id(value: &str) -> Refusal {
    Refusal::DuplicateId { id: id(value) }
}

/// Inserts one element through the `test-support` seam, or fails the test as
/// a setup error.
fn insert(doc: &mut Document, parent: Option<&str>, kind: ElementKind, value: &str) {
    let parent = parent.map(ElementId::from);
    if let Err(error) = doc.insert(parent.as_ref(), kind, id(value), None) {
        panic!("{ROW}: setup could not insert the {kind} {value:?}: {error}");
    }
}

/// Reads `roots` (XMI root elements) in an `xmi:XMI` wrapper.
fn read_file(roots: &str) -> Document {
    let input = format!("{WRAPPER_OPEN}{roots}\n</xmi:XMI>\n");
    match xmi::read(input.as_bytes(), &ReadOptions::default()) {
        Ok(output) => output.into_document(),
        Err(error) => panic!("{ROW}: setup could not read the file: {error}"),
    }
}

fn write(doc: &Document) -> String {
    match xmi::write(doc, &WriteOptions::default()) {
        Ok(text) => text,
        Err(error) => panic!("{ROW}: the document could not be written: {error}"),
    }
}

/// Asserts that `doc` has the element `value` of `kind` under `parent` (a root
/// when `None`) with exactly `name`.
fn assert_element(
    doc: &Document,
    value: &str,
    kind: ElementKind,
    parent: Option<&str>,
    name: &LangString,
) {
    let Some(element) = doc.get(value) else {
        panic!("{ROW}: the document has no element {value:?}");
    };
    assert_eq!(element.kind(), kind, "{ROW}: {value:?} has the wrong kind");
    assert_eq!(
        element.parent().map(|p| p.id().as_str()),
        parent,
        "{ROW}: {value:?} has the wrong parent"
    );
    assert_eq!(
        element.name(),
        Some(name),
        "{ROW}: {value:?} does not have exactly the name given"
    );
}

/// The ids of the roots, in document order.
fn root_ids(doc: &Document) -> Vec<&str> {
    doc.roots().map(|root| root.id().as_str()).collect()
}

/// The ids of the children of `parent`, in order.
fn child_ids<'a>(doc: &'a Document, parent: &str) -> Vec<&'a str> {
    let Some(parent) = doc.get(parent) else {
        panic!("{ROW}: the document has no element {parent:?}");
    };
    parent.children().map(|child| child.id().as_str()).collect()
}

/// The ids the next generated AssuranceCasePackage, ArgumentPackage and Claim
/// would get, observed on a clone so that `doc` is untouched.
fn next_generated_ids(doc: &Document) -> [ElementId; 3] {
    let mut probe = doc.clone();
    let case = create(&mut probe, CreateAssuranceCasePackage::root(named("probe")));
    let arguments = create(&mut probe, CreateArgumentPackage::root(named("probe")));
    let claim = create(&mut probe, CreateClaim::new(&arguments, named("probe")));
    [case, arguments, claim]
}

/// What item 6 compares before and after a refusal.
#[derive(Debug, PartialEq)]
struct Snapshot {
    element_count: usize,
    xmi: String,
    next_ids: [ElementId; 3],
}

fn snapshot(doc: &Document) -> Snapshot {
    Snapshot {
        element_count: doc.element_count(),
        xmi: write(doc),
        next_ids: next_generated_ids(doc),
    }
}

/// Creates an element of `child` (id "child") under a fresh element of
/// `parent_kind` (id "parent"), itself under an ArtifactPackage root, and
/// returns the document and the answer.
fn create_under(parent_kind: ElementKind, child: ElementKind) -> (Document, Result<(), Refusal>) {
    let mut doc = Document::new();
    insert(&mut doc, None, ElementKind::ArtifactPackage, SCAFFOLD);
    insert(&mut doc, Some(SCAFFOLD), parent_kind, "parent");
    let name = named("Child");
    let operation: Operation = match child {
        ElementKind::AssuranceCasePackage => CreateAssuranceCasePackage::new("parent", name)
            .with_id("child")
            .into(),
        ElementKind::ArgumentPackage => CreateArgumentPackage::new("parent", name)
            .with_id("child")
            .into(),
        ElementKind::Claim => CreateClaim::new("parent", name).with_id("child").into(),
        other => panic!("{ROW}: the edit API does not create a {other}"),
    };
    let answer = edit::apply(&mut doc, operation)
        .map(|mutation| {
            assert_eq!(
                mutation.created_ids(),
                [id("child")],
                "{ROW}: created_ids() of a {child} under a {parent_kind}"
            );
        })
        .map_err(|error| refusal_of(&error));
    (doc, answer)
}

/// Asserts that a `child` is created under exactly the parent kinds of
/// `legal_parents(child)`, and refused as `InvalidParent` under every other.
fn assert_parent_legality(child: ElementKind) {
    assert_eq!(
        ElementKind::ALL.len(),
        38,
        "{ROW}: the kinds changed; review this file's legality table"
    );
    let legal = legal_parents(child);
    for &parent_kind in ElementKind::ALL {
        let (doc, answer) = create_under(parent_kind, child);
        if legal.contains(&parent_kind) {
            assert_eq!(
                answer,
                Ok(()),
                "{ROW}: a {child} under a {parent_kind} was refused"
            );
            assert_element(&doc, "child", child, Some("parent"), &named("Child"));
        } else {
            assert_eq!(
                answer,
                Err(invalid_parent("parent", parent_kind, child)),
                "{ROW}: a {child} under a {parent_kind} was not refused as InvalidParent"
            );
            assert_eq!(
                doc.element_count(),
                2,
                "{ROW}: a refused {child} under a {parent_kind} changed the document"
            );
        }
    }
}

// ---------------------------------------------------------------------------
// 1. Each operation creates its element
// ---------------------------------------------------------------------------

#[test]
fn lib_edit_001_create_assurance_case_package_creates_a_root_with_the_given_name_and_id() {
    let mut doc = Document::new();
    let name = named("Safety case");
    let created = create(
        &mut doc,
        CreateAssuranceCasePackage::root(name.clone()).with_id("case"),
    );
    assert_eq!(
        created,
        id("case"),
        "{ROW}: created_ids() is not [\"case\"]"
    );
    assert_element(&doc, "case", ElementKind::AssuranceCasePackage, None, &name);
    assert_eq!(
        doc.element_count(),
        1,
        "{ROW}: more than one element created"
    );
    assert_eq!(root_ids(&doc), ["case"], "{ROW}: the roots");
}

#[test]
fn lib_edit_001_create_argument_package_creates_a_root_with_the_given_name_and_id() {
    let mut doc = Document::new();
    let name = named("Arguments");
    let created = create(
        &mut doc,
        CreateArgumentPackage::root(name.clone()).with_id("arguments"),
    );
    assert_eq!(created, id("arguments"), "{ROW}: created_ids()");
    assert_element(&doc, "arguments", ElementKind::ArgumentPackage, None, &name);
    assert_eq!(root_ids(&doc), ["arguments"], "{ROW}: the roots");
}

#[test]
fn lib_edit_001_package_operations_create_under_the_given_parent() {
    let mut doc = Document::new();
    create(
        &mut doc,
        CreateAssuranceCasePackage::root(named("Case")).with_id("case"),
    );
    let sub_case = named("Sub-case");
    let created = create(
        &mut doc,
        CreateAssuranceCasePackage::new("case", sub_case.clone()).with_id("sub_case"),
    );
    assert_eq!(created, id("sub_case"), "{ROW}: created_ids()");
    assert_element(
        &doc,
        "sub_case",
        ElementKind::AssuranceCasePackage,
        Some("case"),
        &sub_case,
    );
    let arguments = named("Arguments");
    let created = create(
        &mut doc,
        CreateArgumentPackage::new("case", arguments.clone()).with_id("arguments"),
    );
    assert_eq!(created, id("arguments"), "{ROW}: created_ids()");
    assert_element(
        &doc,
        "arguments",
        ElementKind::ArgumentPackage,
        Some("case"),
        &arguments,
    );
    let nested = named("Nested");
    create(
        &mut doc,
        CreateArgumentPackage::new("arguments", nested.clone()).with_id("nested"),
    );
    assert_element(
        &doc,
        "nested",
        ElementKind::ArgumentPackage,
        Some("arguments"),
        &nested,
    );
    assert_eq!(root_ids(&doc), ["case"], "{ROW}: a child became a root");
    assert_eq!(
        child_ids(&doc, "case"),
        ["sub_case", "arguments"],
        "{ROW}: the children of \"case\", in creation order"
    );
}

#[test]
fn lib_edit_001_create_claim_creates_under_the_given_parent() {
    let mut doc = Document::new();
    create(
        &mut doc,
        CreateArgumentPackage::root(named("Arguments")).with_id("arguments"),
    );
    let top = named("The system is acceptably safe");
    let created = create(
        &mut doc,
        CreateClaim::new("arguments", top.clone()).with_id("c_top"),
    );
    assert_eq!(created, id("c_top"), "{ROW}: created_ids()");
    assert_element(&doc, "c_top", ElementKind::Claim, Some("arguments"), &top);
    let second = named("Hazards are mitigated");
    create(
        &mut doc,
        CreateClaim::new("arguments", second.clone()).with_id("c_hazards"),
    );
    assert_element(
        &doc,
        "c_hazards",
        ElementKind::Claim,
        Some("arguments"),
        &second,
    );
    assert_eq!(
        child_ids(&doc, "arguments"),
        ["c_top", "c_hazards"],
        "{ROW}: the claims, in creation order"
    );
    assert_eq!(doc.element_count(), 3, "{ROW}: element count");
}

#[test]
fn lib_edit_001_roots_are_created_in_document_order() {
    let mut doc = Document::new();
    create(
        &mut doc,
        CreateAssuranceCasePackage::root(named("B")).with_id("b"),
    );
    create(
        &mut doc,
        CreateArgumentPackage::root(named("A")).with_id("a"),
    );
    create(
        &mut doc,
        CreateAssuranceCasePackage::root(named("C")).with_id("c"),
    );
    let generated = create(&mut doc, CreateArgumentPackage::root(named("D")));
    assert_eq!(
        root_ids(&doc),
        ["b", "a", "c", generated.as_str()],
        "{ROW}: roots are not in creation order"
    );
}

#[test]
fn lib_edit_001_names_are_stored_exactly() {
    let names = [
        LangString::default(),
        LangString::new(""),
        lang_string(Some(""), None),
        lang_string(Some(""), Some("")),
        lang_string(Some("en"), Some("")),
        lang_string(Some("sv"), None),
        LangString::new("  padded  \t name  ").with_lang("sv"),
        LangString::new("Säkerhetsfall <&> \"quoted\"").with_lang("sv-SE"),
    ];
    for name in &names {
        let mut doc = Document::new();
        let case = create(&mut doc, CreateAssuranceCasePackage::root(name.clone()));
        let sub = create(
            &mut doc,
            CreateAssuranceCasePackage::new(&case, name.clone()),
        );
        let arguments = create(&mut doc, CreateArgumentPackage::root(name.clone()));
        let nested = create(&mut doc, CreateArgumentPackage::new(&case, name.clone()));
        let claim = create(&mut doc, CreateClaim::new(&arguments, name.clone()));
        for created in [&case, &sub, &arguments, &nested, &claim] {
            let stored = doc.get(created).and_then(SacmElement::name);
            assert_eq!(
                stored,
                Some(name),
                "{ROW}: {created:?} does not have exactly the name {name:?}"
            );
        }
    }
}

#[test]
fn lib_edit_001_operations_report_their_parent_id_and_name() {
    let name = named("N");
    let case_root = CreateAssuranceCasePackage::root(name.clone());
    assert_eq!(case_root.parent(), None, "{ROW}: a root operation's parent");
    assert_eq!(case_root.id(), None, "{ROW}: id() without with_id");
    assert_eq!(case_root.name(), &name, "{ROW}: name()");
    let case_root = case_root.with_id("case");
    assert_eq!(
        case_root.id(),
        Some(&id("case")),
        "{ROW}: id() after with_id"
    );

    // A parent given as `&ElementId`.
    let outer = id("outer");
    let case_child = CreateAssuranceCasePackage::new(&outer, name.clone()).with_id("x");
    assert_eq!(case_child.parent(), Some(&outer), "{ROW}: parent()");
    assert_eq!(case_child.id(), Some(&id("x")), "{ROW}: id()");

    let arguments_root = CreateArgumentPackage::root(name.clone());
    assert_eq!(
        arguments_root.parent(),
        None,
        "{ROW}: a root operation's parent"
    );
    assert_eq!(arguments_root.id(), None, "{ROW}: id() without with_id");
    assert_eq!(arguments_root.name(), &name, "{ROW}: name()");
    let arguments_child = CreateArgumentPackage::new("case", name.clone()).with_id("a");
    assert_eq!(
        arguments_child.parent(),
        Some(&id("case")),
        "{ROW}: parent()"
    );
    assert_eq!(arguments_child.id(), Some(&id("a")), "{ROW}: id()");

    let claim = CreateClaim::new("arguments", name.clone());
    assert_eq!(
        claim.parent(),
        &id("arguments"),
        "{ROW}: CreateClaim parent()"
    );
    assert_eq!(claim.id(), None, "{ROW}: id() without with_id");
    assert_eq!(claim.name(), &name, "{ROW}: name()");
    assert_eq!(
        claim.with_id("c1").id(),
        Some(&id("c1")),
        "{ROW}: id() after with_id"
    );

    // Each operation converts to an `Operation`, which `apply` takes too.
    let mut doc = Document::new();
    let operation = Operation::from(case_root);
    assert_eq!(
        create(&mut doc, operation),
        id("case"),
        "{ROW}: Operation::from"
    );
}

// ---------------------------------------------------------------------------
// 2. Generated ids
// ---------------------------------------------------------------------------

#[test]
fn lib_edit_001_generated_ids_count_up_per_kind() {
    let mut doc = Document::new();
    let mut created = Vec::new();
    created.push(create(
        &mut doc,
        CreateAssuranceCasePackage::root(named("1")),
    ));
    created.push(create(&mut doc, CreateArgumentPackage::root(named("2"))));
    let first_arguments = created[1].clone();
    created.push(create(
        &mut doc,
        CreateClaim::new(&first_arguments, named("3")),
    ));
    created.push(create(
        &mut doc,
        CreateClaim::new(&first_arguments, named("4")),
    ));
    let first_case = created[0].clone();
    created.push(create(
        &mut doc,
        CreateArgumentPackage::new(&first_case, named("5")),
    ));
    created.push(create(
        &mut doc,
        CreateAssuranceCasePackage::new(&first_case, named("6")),
    ));
    created.push(create(
        &mut doc,
        CreateClaim::new(&first_arguments, named("7")),
    ));
    let expected: Vec<ElementId> = [
        "assurance_case_package_1",
        "argument_package_1",
        "claim_1",
        "claim_2",
        "argument_package_2",
        "assurance_case_package_2",
        "claim_3",
    ]
    .into_iter()
    .map(id)
    .collect();
    assert_eq!(created, expected, "{ROW}: the generated ids");
}

#[test]
fn lib_edit_001_generation_skips_an_id_given_by_a_caller() {
    let mut doc = Document::new();
    create(
        &mut doc,
        CreateArgumentPackage::root(named("A")).with_id("arguments"),
    );
    // A caller takes claim_2, and an ArgumentPackage takes claim_4: ids are
    // unique across kinds.
    create(
        &mut doc,
        CreateClaim::new("arguments", named("C")).with_id("claim_2"),
    );
    create(
        &mut doc,
        CreateArgumentPackage::new("arguments", named("P")).with_id("claim_4"),
    );
    let generated: Vec<ElementId> = (0..3)
        .map(|_| create(&mut doc, CreateClaim::new("arguments", named("G"))))
        .collect();
    assert_eq!(
        generated,
        [id("claim_1"), id("claim_3"), id("claim_5")],
        "{ROW}: generation did not skip the ids in use"
    );
}

#[test]
fn lib_edit_001_a_caller_id_does_not_advance_the_counter() {
    let mut doc = Document::new();
    create(
        &mut doc,
        CreateAssuranceCasePackage::root(named("Caller")).with_id("assurance_case_package_7"),
    );
    create(
        &mut doc,
        CreateArgumentPackage::root(named("A")).with_id("arguments"),
    );
    create(
        &mut doc,
        CreateClaim::new("arguments", named("Caller")).with_id("claim_9"),
    );
    assert_eq!(
        next_generated_ids(&doc),
        [
            id("assurance_case_package_1"),
            id("argument_package_1"),
            id("claim_1")
        ],
        "{ROW}: a caller id advanced the counter"
    );
}

#[test]
fn lib_edit_001_generation_skips_an_id_read_from_a_file() {
    let mut doc = read_file(concat!(
        "<sacm:ArgumentPackage xmi:id=\"argument_package_1\"/>\n",
        "<sacm:ArgumentPackage xmi:id=\"argument_package_3\"/>\n",
        "<sacm:AssuranceCasePackage xmi:id=\"claim_1\"/>",
    ));
    let first = create(&mut doc, CreateArgumentPackage::root(named("G")));
    let second = create(&mut doc, CreateArgumentPackage::root(named("G")));
    let claim = create(&mut doc, CreateClaim::new("argument_package_1", named("G")));
    let case = create(&mut doc, CreateAssuranceCasePackage::root(named("G")));
    assert_eq!(
        [first, second, claim, case],
        [
            id("argument_package_2"),
            id("argument_package_4"),
            id("claim_2"),
            id("assurance_case_package_1")
        ],
        "{ROW}: generation did not skip the ids read from the file"
    );
}

#[test]
fn lib_edit_001_generation_skips_several_used_ids_in_a_row() {
    let mut doc = Document::new();
    create(
        &mut doc,
        CreateArgumentPackage::root(named("A")).with_id("arguments"),
    );
    for value in ["claim_1", "claim_2", "claim_3"] {
        create(
            &mut doc,
            CreateClaim::new("arguments", named("Caller")).with_id(value),
        );
    }
    let generated = create(&mut doc, CreateClaim::new("arguments", named("G")));
    assert_eq!(
        generated,
        id("claim_4"),
        "{ROW}: generation did not skip three used ids in a row"
    );
}

#[test]
fn lib_edit_001_generation_finds_the_free_id_when_every_element_is_a_candidate() {
    for k in [1, 3] {
        let roots: String = (1..=k)
            .map(|n| {
                [
                    "<sacm:ArgumentPackage xmi:id=\"argument_package_",
                    &n.to_string(),
                    "\"/>\n",
                ]
                .concat()
            })
            .collect();
        let mut doc = read_file(&roots);
        assert_eq!(
            doc.element_count(),
            k,
            "{ROW}: setup: the file is not exactly the {k} candidates"
        );
        let generated = create(&mut doc, CreateArgumentPackage::root(named("G")));
        assert_eq!(
            generated,
            id(&format!("argument_package_{}", k + 1)),
            "{ROW}: with argument_package_1..={k} in use, generation did not find the next id"
        );
    }
}

// ---------------------------------------------------------------------------
// 3. Determinism
// ---------------------------------------------------------------------------

/// A mixed sequence of operations, with generated ids and the caller id
/// `caller` for one Claim; returns every id created, in order.
fn build(doc: &mut Document, caller: &str) -> Vec<ElementId> {
    let mut created = Vec::new();
    let case = create(doc, CreateAssuranceCasePackage::root(named("Case")));
    created.push(case.clone());
    let arguments = create(doc, CreateArgumentPackage::new(&case, named("Arguments")));
    created.push(arguments.clone());
    created.push(create(
        doc,
        CreateClaim::new(&arguments, named("Top")).with_id(caller),
    ));
    for content in ["G1", "G2", "G3"] {
        created.push(create(doc, CreateClaim::new(&arguments, named(content))));
    }
    created.push(create(doc, CreateArgumentPackage::root(named("More"))));
    created
}

#[test]
fn lib_edit_001_equal_documents_given_the_same_operations_generate_the_same_ids() {
    fn from_file() -> Document {
        read_file("<sacm:ArgumentPackage xmi:id=\"argument_package_2\"/>")
    }
    type Start = fn() -> Document;
    let starts: [(&str, Start); 2] = [
        ("an empty document", Document::new),
        ("a document read from a file", from_file),
    ];
    for (what, start) in starts {
        let mut first = start();
        let mut second = start();
        // claim_2, a caller id that generation must skip.
        let first_ids = build(&mut first, "claim_2");
        let second_ids = build(&mut second, "claim_2");
        assert_eq!(
            first_ids, second_ids,
            "{ROW}: {what}: the same operations generated different ids"
        );
        assert_eq!(
            write(&first),
            write(&second),
            "{ROW}: {what}: the same operations gave different documents"
        );
    }
}

#[test]
fn lib_edit_001_a_clone_generates_the_same_ids_as_its_original() {
    let mut original = Document::new();
    build(&mut original, "claim_2");
    let mut copy = original.clone();
    // A counter shared between the two, or global, would differ here.
    let from_original = build(&mut original, "claim_9");
    let from_copy = build(&mut copy, "claim_9");
    assert_eq!(
        from_original, from_copy,
        "{ROW}: a clone generated different ids from its original"
    );
    assert_eq!(
        write(&original),
        write(&copy),
        "{ROW}: the documents differ"
    );
}

// ---------------------------------------------------------------------------
// 4. Parent legality equals the containment table
// ---------------------------------------------------------------------------

#[test]
fn lib_edit_001_an_assurance_case_package_is_created_exactly_under_its_legal_parents() {
    assert_parent_legality(ElementKind::AssuranceCasePackage);
}

#[test]
fn lib_edit_001_an_argument_package_is_created_exactly_under_its_legal_parents() {
    assert_parent_legality(ElementKind::ArgumentPackage);
}

#[test]
fn lib_edit_001_a_claim_is_created_exactly_under_its_legal_parents() {
    assert_parent_legality(ElementKind::Claim);
}

// ---------------------------------------------------------------------------
// 5. Refusals and their order
// ---------------------------------------------------------------------------

/// A document with an AssuranceCasePackage "case" holding an ArgumentPackage
/// "arguments" holding a Claim "c1".
fn small_case() -> Document {
    let mut doc = Document::new();
    create(
        &mut doc,
        CreateAssuranceCasePackage::root(named("Case")).with_id("case"),
    );
    create(
        &mut doc,
        CreateArgumentPackage::new("case", named("Arguments")).with_id("arguments"),
    );
    create(
        &mut doc,
        CreateClaim::new("arguments", named("C1")).with_id("c1"),
    );
    doc
}

#[test]
fn lib_edit_001_an_unknown_parent_is_parent_not_found() {
    let mut doc = small_case();
    let refusals = [
        refuse(
            &mut doc,
            CreateAssuranceCasePackage::new("nowhere", named("X")),
        ),
        refuse(&mut doc, CreateArgumentPackage::new("nowhere", named("X"))),
        refuse(
            &mut doc,
            CreateClaim::new("nowhere", named("X")).with_id("x"),
        ),
        // Ids are case-sensitive and exact.
        refuse(&mut doc, CreateClaim::new("Arguments", named("X"))),
        refuse(&mut doc, CreateClaim::new(" arguments", named("X"))),
    ];
    let parents = ["nowhere", "nowhere", "nowhere", "Arguments", " arguments"];
    for (error, parent) in refusals.iter().zip(parents) {
        assert_eq!(
            refusal_of(error),
            parent_not_found(parent),
            "{ROW}: an unknown parent {parent:?}"
        );
    }
}

#[test]
fn lib_edit_001_an_illegal_parent_is_invalid_parent_with_its_id_and_kind_and_the_child_kind() {
    let mut doc = small_case();
    let cases = [
        (
            refuse(&mut doc, CreateClaim::new("case", named("X"))),
            invalid_parent(
                "case",
                ElementKind::AssuranceCasePackage,
                ElementKind::Claim,
            ),
        ),
        (
            refuse(
                &mut doc,
                CreateAssuranceCasePackage::new("arguments", named("X")),
            ),
            invalid_parent(
                "arguments",
                ElementKind::ArgumentPackage,
                ElementKind::AssuranceCasePackage,
            ),
        ),
        (
            refuse(&mut doc, CreateArgumentPackage::new("c1", named("X"))),
            invalid_parent("c1", ElementKind::Claim, ElementKind::ArgumentPackage),
        ),
        (
            refuse(&mut doc, CreateClaim::new("c1", named("X")).with_id("x")),
            invalid_parent("c1", ElementKind::Claim, ElementKind::Claim),
        ),
    ];
    for (error, expected) in &cases {
        assert_eq!(&refusal_of(error), expected, "{ROW}: an illegal parent");
    }
}

#[test]
fn lib_edit_001_an_id_that_is_not_an_ncname_is_invalid_id() {
    let mut doc = small_case();
    for value in ["", "1", "a b", "a:b", "-x", ".x", "1a", "a\tb"] {
        let operations: [Operation; 5] = [
            CreateAssuranceCasePackage::root(named("X"))
                .with_id(value)
                .into(),
            CreateAssuranceCasePackage::new("case", named("X"))
                .with_id(value)
                .into(),
            CreateArgumentPackage::root(named("X"))
                .with_id(value)
                .into(),
            CreateArgumentPackage::new("case", named("X"))
                .with_id(value)
                .into(),
            CreateClaim::new("arguments", named("X"))
                .with_id(value)
                .into(),
        ];
        for operation in operations {
            let error = refuse(&mut doc, operation);
            assert_eq!(
                refusal_of(&error),
                invalid_id(value),
                "{ROW}: the id {value:?} is not an NCName"
            );
        }
    }
}

#[test]
fn lib_edit_001_an_ncname_id_is_accepted_exactly() {
    let mut doc = small_case();
    for value in [
        "_",
        "a",
        "A1",
        "_x.y-z_1",
        "Säkerhet",
        "x\u{00B7}y",
        "claim_01",
    ] {
        let created = create(
            &mut doc,
            CreateClaim::new("arguments", named("X")).with_id(value),
        );
        assert_eq!(created, id(value), "{ROW}: the NCName {value:?}");
        assert!(
            doc.contains(value),
            "{ROW}: the NCName {value:?} is not stored exactly"
        );
    }
}

/// What `edit::apply` answers for a Claim with the caller id `value` under
/// "arguments" in `doc`.
fn caller_id_answer(doc: &mut Document, value: &str) -> Result<(), Refusal> {
    edit::apply(
        doc,
        CreateClaim::new("arguments", named("X")).with_id(value),
    )
    .map(|_| ())
    .map_err(|error| refusal_of(&error))
}

/// The edges of every range of XML 1.0 (5th edition) production [4]
/// `NameStartChar`, without `:`, which `NCName` excludes.
const NAME_START_EDGES: [char; 29] = [
    'A',
    'Z',
    '_',
    'a',
    'z',
    '\u{C0}',
    '\u{D6}',
    '\u{D8}',
    '\u{F6}',
    '\u{F8}',
    '\u{2FF}',
    '\u{370}',
    '\u{37D}',
    '\u{37F}',
    '\u{1FFF}',
    '\u{200C}',
    '\u{200D}',
    '\u{2070}',
    '\u{218F}',
    '\u{2C00}',
    '\u{2FEF}',
    '\u{3001}',
    '\u{D7FF}',
    '\u{F900}',
    '\u{FDCF}',
    '\u{FDF0}',
    '\u{FFFD}',
    '\u{10000}',
    '\u{EFFFF}',
];

/// The characters just outside each `NameStartChar` edge that no neighbouring
/// range covers, and the `NameChar`s of [4a] that cannot start a name.
/// U+D800-U+DFFF, after U+D7FF, are surrogates and not `char`s, so U+E000 is
/// the next character there.
const NOT_NAME_START: [char; 33] = [
    ':',
    '@',
    '[',
    '^',
    '`',
    '{',
    '\u{BF}',
    '\u{D7}',
    '\u{F7}',
    '\u{36F}',
    '\u{37E}',
    '\u{2000}',
    '\u{200B}',
    '\u{200E}',
    '\u{206F}',
    '\u{2190}',
    '\u{2BFF}',
    '\u{2FF0}',
    '\u{3000}',
    '\u{E000}',
    '\u{F8FF}',
    '\u{FDD0}',
    '\u{FDEF}',
    '\u{FFFE}',
    '\u{FFFF}',
    '\u{F0000}', // NameChars that are not NameStartChars:
    '-',
    '.',
    '0',
    '9',
    '\u{B7}',
    '\u{300}',
    '\u{203F}',
];

#[test]
fn lib_edit_001_ncname_first_characters_are_checked_at_every_range_edge() {
    let mut doc = small_case();
    for first in NAME_START_EDGES {
        let value = first.to_string();
        assert_eq!(
            caller_id_answer(&mut doc, &value),
            Ok(()),
            "{ROW}: U+{:04X} is a NameStartChar, but the id {value:?} was refused",
            u32::from(first)
        );
        assert!(
            doc.contains(&value),
            "{ROW}: the id {value:?} is not stored"
        );
    }
    for first in NOT_NAME_START {
        let value = format!("{first}x");
        assert_eq!(
            caller_id_answer(&mut doc, &value),
            Err(invalid_id(&value)),
            "{ROW}: U+{:04X} cannot start an NCName, but the id {value:?} was not InvalidId",
            u32::from(first)
        );
    }
}

#[test]
fn lib_edit_001_ncname_later_characters_are_checked_at_every_range_edge() {
    let mut doc = small_case();
    // The edges of the ranges [4a] adds to [4], and a NameStartChar.
    for later in [
        '-', '.', '0', '9', '\u{B7}', '\u{300}', '\u{36F}', '\u{203F}', '\u{2040}', '\u{C0}',
    ] {
        let value = format!("a{later}");
        assert_eq!(
            caller_id_answer(&mut doc, &value),
            Ok(()),
            "{ROW}: U+{:04X} is a NameChar, but the id {value:?} was refused",
            u32::from(later)
        );
    }
    // Just outside each of those edges, where no range covers it.
    for later in [
        ',', '/', ':', '\u{B6}', '\u{B8}', '\u{37E}', '\u{203E}', '\u{2041}', ' ',
    ] {
        let value = format!("a{later}");
        assert_eq!(
            caller_id_answer(&mut doc, &value),
            Err(invalid_id(&value)),
            "{ROW}: U+{:04X} is not a NameChar, but the id {value:?} was not InvalidId",
            u32::from(later)
        );
    }
}

#[test]
fn lib_edit_001_an_id_already_in_the_document_is_duplicate_id() {
    let mut doc = small_case();
    // A kind the edit API does not create holds an id too.
    insert(
        &mut doc,
        Some("case"),
        ElementKind::ArtifactPackage,
        "evidence",
    );
    let cases = [
        refuse(
            &mut doc,
            CreateAssuranceCasePackage::root(named("X")).with_id("case"),
        ),
        refuse(
            &mut doc,
            CreateArgumentPackage::root(named("X")).with_id("c1"),
        ),
        refuse(
            &mut doc,
            CreateArgumentPackage::new("case", named("X")).with_id("evidence"),
        ),
        refuse(
            &mut doc,
            CreateClaim::new("arguments", named("X")).with_id("arguments"),
        ),
        refuse(
            &mut doc,
            CreateClaim::new("arguments", named("X")).with_id("c1"),
        ),
    ];
    let ids = ["case", "c1", "evidence", "arguments", "c1"];
    for (error, value) in cases.iter().zip(ids) {
        assert_eq!(
            refusal_of(error),
            duplicate_id(value),
            "{ROW}: the id {value:?} is in use"
        );
    }
}

#[test]
fn lib_edit_001_an_id_read_from_a_file_is_duplicate_id() {
    let mut doc = read_file(concat!(
        "<sacm:AssuranceCasePackage xmi:id=\"from_file\"/>\n",
        "<sacm:ArgumentPackage xmi:id=\"argument_package_1\"/>",
    ));
    let cases = [
        refuse(
            &mut doc,
            CreateArgumentPackage::root(named("X")).with_id("from_file"),
        ),
        refuse(
            &mut doc,
            CreateAssuranceCasePackage::new("from_file", named("X")).with_id("argument_package_1"),
        ),
        refuse(
            &mut doc,
            CreateClaim::new("argument_package_1", named("X")).with_id("from_file"),
        ),
    ];
    let ids = ["from_file", "argument_package_1", "from_file"];
    for (error, value) in cases.iter().zip(ids) {
        assert_eq!(
            refusal_of(error),
            duplicate_id(value),
            "{ROW}: the id {value:?} read from the file"
        );
    }
    // A generated id that would collide with it is skipped, not refused.
    let generated = create(&mut doc, CreateArgumentPackage::root(named("G")));
    assert_eq!(
        generated,
        id("argument_package_2"),
        "{ROW}: the generated id did not skip the id read from the file"
    );
}

#[test]
fn lib_edit_001_refusals_are_checked_in_order() {
    let mut doc = small_case();
    // Ids that are not NCNames, held exactly as read or inserted: only new
    // ids are checked.
    insert(&mut doc, Some("case"), ElementKind::ArgumentPackage, "a b");
    let cases = [
        // 1 before 2-4: an unknown parent, with a bad id and a duplicate id.
        (
            refuse(
                &mut doc,
                CreateClaim::new("nowhere", named("X")).with_id("1"),
            ),
            parent_not_found("nowhere"),
        ),
        (
            refuse(
                &mut doc,
                CreateClaim::new("nowhere", named("X")).with_id("c1"),
            ),
            parent_not_found("nowhere"),
        ),
        // 2 before 3 and 4: an illegal parent, with a bad id and a duplicate id.
        (
            refuse(
                &mut doc,
                CreateClaim::new("case", named("X")).with_id("a:b"),
            ),
            invalid_parent(
                "case",
                ElementKind::AssuranceCasePackage,
                ElementKind::Claim,
            ),
        ),
        (
            refuse(
                &mut doc,
                CreateAssuranceCasePackage::new("c1", named("X")).with_id("case"),
            ),
            invalid_parent("c1", ElementKind::Claim, ElementKind::AssuranceCasePackage),
        ),
        // 3 before 4: an id that is not an NCName and is also in use.
        (
            refuse(
                &mut doc,
                CreateArgumentPackage::new("case", named("X")).with_id("a b"),
            ),
            invalid_id("a b"),
        ),
    ];
    for (error, expected) in &cases {
        assert_eq!(
            &refusal_of(error),
            expected,
            "{ROW}: the first failing check"
        );
    }

    let mut from_file = read_file("<sacm:AssuranceCasePackage xmi:id=\"1a\"/>");
    let error = refuse(
        &mut from_file,
        CreateAssuranceCasePackage::root(named("X")).with_id("1a"),
    );
    assert_eq!(
        refusal_of(&error),
        invalid_id("1a"),
        "{ROW}: a non-NCName id read from a file and given again"
    );
}

// ---------------------------------------------------------------------------
// 6. A refused operation leaves the document unchanged
// ---------------------------------------------------------------------------

#[test]
fn lib_edit_001_a_refused_operation_leaves_the_document_unchanged() {
    let mut doc = small_case();
    // Advance the counters, so that a refusal that moved them would show.
    create(&mut doc, CreateAssuranceCasePackage::root(named("G")));
    create(&mut doc, CreateArgumentPackage::root(named("G")));
    create(&mut doc, CreateClaim::new("arguments", named("G")));
    let refused: Vec<Operation> = vec![
        // With generated ids: a counter that moved would show.
        CreateAssuranceCasePackage::new("nowhere", named("X")).into(),
        CreateArgumentPackage::new("nowhere", named("X")).into(),
        CreateClaim::new("nowhere", named("X")).into(),
        CreateAssuranceCasePackage::new("arguments", named("X")).into(),
        CreateArgumentPackage::new("c1", named("X")).into(),
        CreateClaim::new("case", named("X")).into(),
        // With caller ids.
        CreateAssuranceCasePackage::root(named("X"))
            .with_id("a b")
            .into(),
        CreateArgumentPackage::new("case", named("X"))
            .with_id("")
            .into(),
        CreateClaim::new("arguments", named("X"))
            .with_id("-x")
            .into(),
        CreateAssuranceCasePackage::root(named("X"))
            .with_id("case")
            .into(),
        CreateArgumentPackage::root(named("X")).with_id("c1").into(),
        CreateClaim::new("arguments", named("X"))
            .with_id("arguments")
            .into(),
    ];
    let before = snapshot(&doc);
    for operation in refused {
        let error = refuse(&mut doc, operation);
        assert_eq!(
            snapshot(&doc),
            before,
            "{ROW}: the refusal {error:?} changed the document"
        );
    }
}

// ---------------------------------------------------------------------------
// 7. The error contract
// ---------------------------------------------------------------------------

/// One `EditError` of each variant, each produced by `edit::apply`, with the
/// code and the `Display` text it must have.
fn every_edit_error() -> Vec<(EditError, Code, &'static str)> {
    let mut doc = Document::new();
    create(
        &mut doc,
        CreateAssuranceCasePackage::root(named("X")).with_id("x"),
    );
    vec![
        (
            refuse(&mut doc, CreateClaim::new("y", named("X"))),
            Code::CMD_TARGET_NOT_FOUND,
            "the document has no element \"y\" to be the parent",
        ),
        (
            refuse(&mut doc, CreateClaim::new("a\"b", named("X"))),
            Code::CMD_TARGET_NOT_FOUND,
            "the document has no element \"a\\\"b\" to be the parent",
        ),
        (
            refuse(&mut doc, CreateClaim::new("x", named("X"))),
            Code::CMD_INVALID_PARENT,
            "element \"x\" of kind AssuranceCasePackage cannot contain an element of kind Claim",
        ),
        (
            refuse(
                &mut doc,
                CreateArgumentPackage::root(named("X")).with_id("1a"),
            ),
            Code::ID_INVALID,
            "the element id \"1a\" is not an XML NCName",
        ),
        (
            refuse(
                &mut doc,
                CreateArgumentPackage::root(named("X")).with_id("x"),
            ),
            Code::CMD_DUPLICATE_ID,
            "the document already has an element with id \"x\"",
        ),
    ]
}

#[test]
fn lib_edit_001_edit_error_codes_are_the_catalog_codes() {
    for (code, text) in [
        (Code::CMD_TARGET_NOT_FOUND, "SACM-CMD-001"),
        (Code::CMD_DUPLICATE_ID, "SACM-CMD-004"),
        (Code::CMD_INVALID_PARENT, "SACM-CMD-005"),
        (Code::ID_INVALID, "SACM-ID-002"),
    ] {
        assert_eq!(code.as_str(), text, "{ROW}: the code {code:?}");
    }
    for (error, code, _) in every_edit_error() {
        assert_eq!(error.code(), code, "{ROW}: the code of {error:?}");
    }
}

#[test]
fn lib_edit_001_edit_error_display_is_one_lowercase_line() {
    for (error, _, text) in every_edit_error() {
        let shown = error.to_string();
        assert_eq!(shown, text, "{ROW}: the Display of {error:?}");
        assert!(
            !shown.contains(['\n', '\r']),
            "{ROW}: the Display of {error:?} is not one line"
        );
        assert!(
            shown.starts_with(|c: char| c.is_lowercase()),
            "{ROW}: the Display of {error:?} does not start in lower case"
        );
    }
}

fn assert_error_type<T: Error + Send + Sync + 'static>() {}

#[test]
fn lib_edit_001_edit_error_is_a_send_sync_static_error_without_a_source() {
    assert_error_type::<EditError>();
    for (error, _, _) in every_edit_error() {
        assert!(error.source().is_none(), "{ROW}: {error:?} has a source");
        let boxed: Box<dyn Error + Send + Sync + 'static> = Box::new(error);
        assert!(
            boxed.source().is_none(),
            "{ROW}: a boxed error has a source"
        );
    }
}

#[test]
fn lib_edit_001_each_edit_error_code_has_an_error_row_in_the_catalog() {
    let path = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../docs/sacm/diagnostics.md");
    let catalog = std::fs::read_to_string(&path)
        .unwrap_or_else(|error| panic!("{ROW}: cannot read {}: {error}", path.display()));
    for code in [
        Code::CMD_TARGET_NOT_FOUND,
        Code::CMD_INVALID_PARENT,
        Code::ID_INVALID,
        Code::CMD_DUPLICATE_ID,
    ] {
        let cell = format!("| {} |", code.as_str());
        let rows: Vec<&str> = catalog
            .lines()
            .map(str::trim_start)
            .filter(|line| line.starts_with(&cell))
            .collect();
        let [line] = rows.as_slice() else {
            panic!(
                "{ROW}: docs/sacm/diagnostics.md has {} rows for {code:?}, not one",
                rows.len()
            );
        };
        assert!(
            line.starts_with(&format!("| {} | Error |", code.as_str())),
            "{ROW}: the catalog row for {code:?} is not an Error: {line}"
        );
    }
}

// ---------------------------------------------------------------------------
// 8. A document built through the edit API validates, writes and reads back
// ---------------------------------------------------------------------------

#[test]
fn lib_edit_001_a_document_built_by_edit_validates_and_writes() {
    let mut doc = Document::new();
    let case = create(&mut doc, CreateAssuranceCasePackage::root(named("Case")));
    let sub = create(
        &mut doc,
        CreateAssuranceCasePackage::new(&case, named("Sub")),
    );
    let arguments = create(
        &mut doc,
        CreateArgumentPackage::new(&sub, named("Arguments")),
    );
    create(&mut doc, CreateClaim::new(&arguments, named("Top")));
    create(
        &mut doc,
        CreateClaim::new(&arguments, named("Hazards")).with_id("c_hazards"),
    );
    let root_arguments = create(&mut doc, CreateArgumentPackage::root(named("More")));
    create(&mut doc, CreateClaim::new(&root_arguments, named("Other")));
    assert_eq!(doc.element_count(), 7, "{ROW}: element count");
    let findings = validation::validate(&doc);
    assert!(
        findings.is_empty(),
        "{ROW}: a document built by edit::apply has findings: {findings:?}"
    );
    let text = write(&doc);
    assert!(
        text.contains("xmi:id=\"c_hazards\""),
        "{ROW}: the written XMI lacks a created claim:\n{text}"
    );
}

#[test]
fn lib_edit_001_roots_built_by_edit_read_back_with_the_same_ids_kinds_and_names() {
    let mut doc = Document::new();
    create(
        &mut doc,
        CreateAssuranceCasePackage::root(named("Safety case")).with_id("case"),
    );
    create(
        &mut doc,
        CreateArgumentPackage::root(LangString::new("Argument").with_lang("sv")),
    );
    create(&mut doc, CreateAssuranceCasePackage::root(named("Second")));
    let text = write(&doc);
    let read = match xmi::read(text.as_bytes(), &ReadOptions::default()) {
        Ok(output) => output.into_document(),
        Err(error) => panic!("{ROW}: the written document does not read back: {error}"),
    };
    let shape = |doc: &Document| -> Vec<(ElementId, ElementKind, Option<LangString>)> {
        doc.roots()
            .map(|root| (root.id().clone(), root.kind(), root.name().cloned()))
            .collect()
    };
    assert_eq!(
        shape(&read),
        shape(&doc),
        "{ROW}: the roots did not read back the same"
    );
    assert_eq!(
        read.element_count(),
        doc.element_count(),
        "{ROW}: element count after reading back"
    );
}
