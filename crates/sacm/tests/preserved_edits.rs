//! Tests for preserved content under the model, validation and edits
//! (LIB-COMPAT-003; ADR 0033; D-PRESERVE; D-POLICY; D-VAL-LAYER).
//!
//! Content that is not valid SACM where it stands is kept, in both modes, as
//! preserved content of the element it sits in (its holder). These tests
//! check what the rest of the library does with it:
//!
//! - a preserved `xmi:id` stays in the document's id space: `get` and
//!   `contains` do not find it, `with_id` of it is `DuplicateId`, an edit
//!   that would reference it is `ReferenceNotFound`, and generated ids skip
//!   it and every id preserved content refers to;
//! - `validate` reports a typed reference to a preserved id as SACM-REF-003
//!   (`Code::REF_PRESERVED_TARGET`, a Warning), never as SACM-REF-001 or
//!   SACM-REF-002; references inside preserved content are opaque;
//! - a preserved child element counts as content for `RejectIfNonEmpty`,
//!   preserved attributes and text do not; deleting a holder removes its
//!   preserved content, frees its ids, and makes typed references to them
//!   dangling references, handled by the reference policy;
//! - a preserved child keeps its index among the typed children under
//!   deletes, reorders and creates, as the tolerant write shows;
//! - `Clone` copies it, and no other edit changes or refuses because of it;
//! - a preserved part is never an occurrence of a single-valued property
//!   (`RepeatedValue`), and content with nowhere to be kept (on `xmi:XMI`,
//!   or a preserved attribute whose `sacm` or `xmi` prefix is bound to
//!   another URI) is `Unsupported`, labelled a defect in the input.
//!
//! Every document is read from XMI written here. The vendor namespace is
//! `urn:example:vendor`, bound to `v` on the root.

use std::collections::BTreeSet;

use sacm::compare::{Difference, semantic_compare};
use sacm::diagnostic::{Code, Diagnostic, Severity};
use sacm::edit::CrossPackageReferencePolicy::ApplyReferencePolicy;
use sacm::edit::PackageDeletePolicy::DeleteRecursively;
use sacm::edit::ReferenceDeletePolicy::ScrubReferences;
use sacm::edit::{
    self, AddReference, CreateArgumentPackage, CreateAssertedInference, CreateClaim, CreateNote,
    DeleteElement, EditError, Mutation, Operation, RemoveReference, ReorderElements, SetFlag,
    SetGid, SetName, SetReferences,
};
use sacm::model::{Document, ElementId, Flag, LangString, ReferenceEnd, SacmElement};
use sacm::validation;
use sacm::xmi::{self, Mode, ReadError, ReadOptions, WriteOptions};

const ROW: &str = "LIB-COMPAT-003";

const MODES: [Mode; 2] = [Mode::Strict, Mode::Tolerant];

const SACM_NS: &str = "http://www.omg.org/spec/SACM/20220301";
const XMI_NS: &str = "http://www.omg.org/spec/XMI/20131001";

/// The namespace declarations of a root: `sacm`, `xmi` and the vendor's `v`.
const NS: &str = concat!(
    " xmlns:sacm=\"http://www.omg.org/spec/SACM/20220301\"",
    " xmlns:xmi=\"http://www.omg.org/spec/XMI/20131001\"",
    " xmlns:v=\"urn:example:vendor\"",
);

// ---------------------------------------------------------------------------
// Documents
// ---------------------------------------------------------------------------

/// An `argumentElement` of the class `kind` with the id `id` and the extra
/// attributes `attributes`, named after its id, holding `body` after its
/// name.
fn element(kind: &str, id: &str, attributes: &str, body: &str) -> String {
    format!(
        "<argumentElement xmi:type=\"sacm:{kind}\" xmi:id=\"{id}\"{attributes}><name content=\"{id}\"/>{body}</argumentElement>"
    )
}

/// A Claim `id` with `attributes`, holding `body`.
fn claim(id: &str, attributes: &str, body: &str) -> String {
    element("Claim", id, attributes, body)
}

/// The root ArgumentPackage `p`, holding `body`.
fn package(body: &str) -> String {
    format!(
        "<sacm:ArgumentPackage{NS} xmi:id=\"p\"><name content=\"p\"/>{body}</sacm:ArgumentPackage>"
    )
}

/// The root AssuranceCasePackage `case`, holding `body`.
fn case(body: &str) -> String {
    format!(
        "<sacm:AssuranceCasePackage{NS} xmi:id=\"case\"><name content=\"case\"/>{body}</sacm:AssuranceCasePackage>"
    )
}

/// An ArgumentPackage `id` in an AssuranceCasePackage, holding `body`.
fn argument_package(id: &str, body: &str) -> String {
    format!("<argumentPackage xmi:id=\"{id}\"><name content=\"{id}\"/>{body}</argumentPackage>")
}

fn named(content: &str) -> LangString {
    LangString::new(content)
}

fn id(value: &str) -> ElementId {
    ElementId::from(value)
}

fn ids(values: &[&str]) -> Vec<ElementId> {
    values.iter().copied().map(ElementId::from).collect()
}

/// Reads `input` in `mode`, which must succeed with only SACM-XMI-003
/// findings (a Warning tolerant, an Error strict).
fn read_in(what: &str, mode: Mode, input: &str) -> Document {
    match xmi::read(input.as_bytes(), &ReadOptions::default().with_mode(mode)) {
        Ok(output) => {
            let expected = match mode {
                Mode::Strict => Severity::Error,
                _ => Severity::Warning,
            };
            for found in output.diagnostics() {
                assert_eq!(
                    (found.code(), found.severity()),
                    (Code::XMI_UNKNOWN_CONTENT, expected),
                    "{ROW}: {what}: setup: a {mode:?} read reported {found:?}"
                );
            }
            output.into_document()
        }
        Err(error) => {
            panic!("{ROW}: {what}: a {mode:?} read refused it: {error:?} ({error})\n{input}")
        }
    }
}

/// Reads `input` in tolerant mode.
fn read(what: &str, input: &str) -> Document {
    read_in(what, Mode::Tolerant, input)
}

/// The tolerant write of `doc`.
fn write(what: &str, doc: &Document) -> String {
    xmi::write(doc, &WriteOptions::default().with_mode(Mode::Tolerant))
        .unwrap_or_else(|error| panic!("{ROW}: {what}: the tolerant write failed: {error:?}"))
}

/// Applies `operation`, which must succeed.
fn apply(what: &str, doc: &mut Document, operation: impl Into<Operation>) -> Mutation {
    edit::apply(doc, operation)
        .unwrap_or_else(|error| panic!("{ROW}: {what}: the edit was refused: {error:?} ({error})"))
}

/// Applies `operation`, which must be refused, leaving `doc` unchanged, and
/// returns the refusal.
fn refuse(what: &str, doc: &mut Document, operation: impl Into<Operation>) -> EditError {
    let before = doc.clone();
    let revision = doc.revision();
    let operation = operation.into();
    let checked = edit::check(doc, &operation);
    let Err(error) = edit::apply(doc, operation) else {
        panic!("{ROW}: {what}: the edit was accepted");
    };
    assert_eq!(
        checked,
        Err(error.clone()),
        "{ROW}: {what}: check and apply disagree"
    );
    assert!(
        *doc == before && doc.revision() == revision,
        "{ROW}: {what}: a refused edit changed the document"
    );
    error
}

/// The tolerant write's offset of `needle`, which must occur once.
fn at(what: &str, out: &str, needle: &str) -> usize {
    assert_eq!(
        out.matches(needle).count(),
        1,
        "{ROW}: {what}: {needle:?} is not written exactly once in\n{out}"
    );
    out.find(needle).unwrap_or_default()
}

/// Asserts that `needles` are written in that order in `out`.
fn assert_written_in_order(what: &str, out: &str, needles: &[&str]) {
    let offsets: Vec<usize> = needles.iter().map(|needle| at(what, out, needle)).collect();
    assert!(
        offsets.windows(2).all(|pair| pair[0] < pair[1]),
        "{ROW}: {what}: {needles:?} are not written in that order in\n{out}"
    );
}

/// The `xmi:id` attribute of `id`, as the writer writes it.
fn id_attribute(value: &str) -> String {
    format!("xmi:id=\"{value}\"")
}

// ---------------------------------------------------------------------------
// Ids: get, contains, with_id, references, generation
// ---------------------------------------------------------------------------

/// A Claim c1 holding the preserved `<v:note xmi:id="a1">`, with a nested
/// `xmi:id="a2"`, and a Claim c2.
fn holder_document() -> String {
    package(&format!(
        "{}{}",
        claim(
            "c1",
            "",
            "<v:note xmi:id=\"a1\"><v:inner xmi:id=\"a2\"/></v:note>"
        ),
        claim("c2", "", "")
    ))
}

#[test]
fn lib_compat_003_get_and_contains_do_not_find_a_preserved_id() {
    for mode in MODES {
        let what = format!("a preserved xmi:id read in {mode:?} mode");
        let doc = read_in(&what, mode, &holder_document());
        for preserved in ["a1", "a2"] {
            assert!(
                doc.get(preserved).is_none() && !doc.contains(preserved),
                "{ROW}: {what}: {preserved:?} is found as an element"
            );
        }
        let elements: Vec<&str> = doc.elements().map(|e| e.id().as_str()).collect();
        assert_eq!(
            (elements, doc.element_count()),
            (vec!["p", "c1", "c2"], 3),
            "{ROW}: {what}: the elements are the typed ones only"
        );
        let holders: Vec<&str> = doc
            .elements()
            .filter(|e| e.has_preserved_content())
            .map(|e| e.id().as_str())
            .collect();
        assert_eq!(holders, ["c1"], "{ROW}: {what}: the holders");
    }
}

#[test]
fn lib_compat_003_with_id_of_a_preserved_id_is_duplicate_id() {
    let mut doc = read("with_id of a preserved id", &holder_document());
    for preserved in ["a1", "a2"] {
        let what = format!("CreateClaim with_id({preserved:?})");
        let error = refuse(
            &what,
            &mut doc,
            CreateClaim::new("p", named("C")).with_id(preserved),
        );
        assert!(
            matches!(&error, EditError::DuplicateId { id, .. } if id.as_str() == preserved),
            "{ROW}: {what}: expected DuplicateId, got {error:?}"
        );
        assert_eq!(
            error.code(),
            Code::CMD_DUPLICATE_ID,
            "{ROW}: {what}: the code"
        );
        let what = format!("CreateArgumentPackage::root with_id({preserved:?})");
        let error = refuse(
            &what,
            &mut doc,
            CreateArgumentPackage::root(named("P")).with_id(preserved),
        );
        assert!(
            matches!(&error, EditError::DuplicateId { id, .. } if id.as_str() == preserved),
            "{ROW}: {what}: expected DuplicateId, got {error:?}"
        );
    }
}

#[test]
fn lib_compat_003_an_edit_referencing_a_preserved_id_is_reference_not_found() {
    let mut doc = read("a reference to a preserved id", &holder_document());
    let cases: Vec<(&str, Operation, &str)> = vec![
        (
            "AddReference metaClaim a1",
            AddReference::new("c2", ReferenceEnd::MetaClaim, "a1").into(),
            "metaClaim",
        ),
        (
            "SetReferences citedElement [a1]",
            SetReferences::new("c2", ReferenceEnd::CitedElement, ["a1"]).into(),
            "citedElement",
        ),
        (
            "SetReferences abstractForm [a2]",
            SetReferences::new("c2", ReferenceEnd::AbstractForm, ["a2"]).into(),
            "abstractForm",
        ),
        (
            "CreateAssertedInference with the source a1",
            CreateAssertedInference::new("p", named("I"), ["a1"], "c2").into(),
            "source",
        ),
        (
            "CreateAssertedInference with the target a1",
            CreateAssertedInference::new("p", named("I"), ["c2"], "a1").into(),
            "target",
        ),
    ];
    for (what, operation, expected_end) in cases {
        let error = refuse(what, &mut doc, operation);
        assert!(
            matches!(
                &error,
                EditError::ReferenceNotFound { end, id, .. }
                    if *end == expected_end && id.as_str().starts_with('a')
            ),
            "{ROW}: {what}: expected ReferenceNotFound on {expected_end}, got {error:?}"
        );
        assert_eq!(
            error.code(),
            Code::CMD_TARGET_NOT_FOUND,
            "{ROW}: {what}: the code"
        );
    }
}

#[test]
fn lib_compat_003_an_end_already_holding_a_preserved_id_keeps_it_when_set_again() {
    // An id an end already holds is not checked again (LIB-EDIT-004), so a
    // reference to a preserved id read from a file can be kept.
    let input = package(&format!(
        "{}{}",
        claim("c1", "", "<v:note xmi:id=\"a1\"/>"),
        claim("c2", " isCitation=\"true\" citedElement=\"a1\"", "")
    ));
    let mut doc = read("an end holding a preserved id", &input);
    let mutation = apply(
        "SetReferences citedElement [a1] again",
        &mut doc,
        SetReferences::new("c2", ReferenceEnd::CitedElement, ["a1"]),
    );
    assert!(
        mutation.changed_ids().is_empty(),
        "{ROW}: setting the id held again changes nothing"
    );
}

/// The id the next generated Claim under `p` gets in `doc`.
fn next_claim_id(doc: &Document) -> ElementId {
    let mut doc = doc.clone();
    let mutation = apply(
        "a generated Claim",
        &mut doc,
        CreateClaim::new("p", named("G")),
    );
    mutation
        .created_ids()
        .first()
        .cloned()
        .unwrap_or_else(|| panic!("{ROW}: a create reported no created id"))
}

#[test]
fn lib_compat_003_generated_ids_skip_preserved_ids_and_preserved_idrefs() {
    let cases = [
        (
            "an xmi:id of a preserved element",
            claim("c1", "", "<v:note xmi:id=\"claim_1\"/>"),
            "claim_2",
        ),
        (
            "an xmi:id nested in a preserved element",
            claim(
                "c1",
                "",
                "<v:note><v:a xmi:id=\"claim_1\"/><v:b xmi:id=\"claim_2\"/></v:note>",
            ),
            "claim_3",
        ),
        (
            "an xmi:idref in a preserved element",
            claim("c1", "", "<v:link xmi:idref=\"claim_1\"/>"),
            "claim_2",
        ),
        (
            "an href=\"#id\" in a preserved element",
            claim("c1", "", "<v:link href=\"#claim_1\"/>"),
            "claim_2",
        ),
        (
            "the tokens of a reference-named attribute in a preserved element",
            claim("c1", "", "<v:link source=\"claim_1 claim_2\"/>"),
            "claim_3",
        ),
        (
            "the tokens of a reference-named attribute the class lacks",
            claim("c1", " source=\"claim_1 claim_2\"", ""),
            "claim_3",
        ),
    ];
    for (what, body, expected) in cases {
        let doc = read(what, &package(&body));
        assert_eq!(
            next_claim_id(&doc),
            id(expected),
            "{ROW}: {what}: the generated id does not skip it"
        );
    }
}

// ---------------------------------------------------------------------------
// Validation: SACM-REF-003
// ---------------------------------------------------------------------------

/// The `(affected, clause)` of `found`, for comparing findings as a set.
fn affected_and_clause(found: &Diagnostic) -> (Vec<ElementId>, Option<String>) {
    (found.affected().to_vec(), found.clause().map(str::to_owned))
}

#[test]
fn lib_compat_003_a_typed_reference_to_a_preserved_id_is_ref_003_on_every_end() {
    let input = package(
        &[
            claim("c1", "", "<v:note xmi:id=\"a1\"/>"),
            claim("c2", " isCitation=\"true\" citedElement=\"a1\"", ""),
            claim("c3", " isAbstract=\"true\" abstractForm=\"a1\"", ""),
            claim("c4", " metaClaim=\"a1\"", ""),
            element(
                "AssertedInference",
                "i1",
                " source=\"a1\" target=\"c1\" reasoning=\"a1\"",
                "",
            ),
            element("ArgumentReasoning", "r1", " structure=\"a1\"", ""),
            element(
                "ArtifactReference",
                "ar1",
                " referencedArtifactElement=\"a1\"",
                "",
            ),
            element("ArgumentGroup", "g1", " argumentElement=\"a1\"", ""),
        ]
        .concat(),
    );
    let doc = read("typed references to a preserved id", &input);
    let found = validation::validate(&doc);
    for other in [Code::REF_DANGLING, Code::REF_WRONG_TYPE] {
        let wrong: Vec<&Diagnostic> = found.iter().filter(|f| f.code() == other).collect();
        assert!(
            wrong.is_empty(),
            "{ROW}: a reference to a preserved id is never {other}: {wrong:?}"
        );
    }
    let preserved: Vec<&Diagnostic> = found
        .iter()
        .filter(|f| f.code() == Code::REF_PRESERVED_TARGET)
        .collect();
    for finding in &preserved {
        assert_eq!(
            (
                finding.code().as_str(),
                finding.severity(),
                finding.requirement_id()
            ),
            ("SACM-REF-003", Severity::Warning, ROW),
            "{ROW}: SACM-REF-003 is a Warning of this row: {finding:?}"
        );
        assert!(
            finding.location().is_none() && !finding.message().contains('\n'),
            "{ROW}: a validation finding has no location and one line: {finding:?}"
        );
    }
    let mut got: Vec<(Vec<ElementId>, Option<String>)> =
        preserved.iter().map(|f| affected_and_clause(f)).collect();
    got.sort();
    let mut expected: Vec<(Vec<ElementId>, Option<String>)> = [
        ("c2", "8.2"),
        ("c3", "8.2"),
        ("c4", "11.10"),
        ("i1", "11.13"),
        ("i1", "11.13"),
        ("r1", "11.12"),
        ("ar1", "11.9"),
        ("g1", "11.2"),
    ]
    .iter()
    .map(|(referrer, clause)| (ids(&[referrer, "a1"]), Some((*clause).to_owned())))
    .collect();
    expected.sort();
    assert_eq!(
        got, expected,
        "{ROW}: one SACM-REF-003 per end naming a preserved id, affecting [referrer, id], with \
         the end's clause"
    );
}

#[test]
fn lib_compat_003_references_inside_preserved_content_are_opaque_and_dangling_ones_stay_ref_001() {
    let input = package(
        &[
            claim(
                "c1",
                " v:target=\"nowhere1\"",
                "<v:link xmi:idref=\"nowhere2\" source=\"nowhere3\"/>",
            ),
            claim("c2", " metaClaim=\"nowhere4\"", ""),
        ]
        .concat(),
    );
    let doc = read("references in preserved content", &input);
    let refs: Vec<(Code, Vec<ElementId>)> = validation::validate(&doc)
        .iter()
        .filter(|f| f.code().as_str().starts_with("SACM-REF-"))
        .map(|f| (f.code(), f.affected().to_vec()))
        .collect();
    assert_eq!(
        refs,
        [(Code::REF_DANGLING, ids(&["c2", "nowhere4"]))],
        "{ROW}: ids inside preserved content are not checked; a typed dangling reference is \
         still SACM-REF-001"
    );
}

// ---------------------------------------------------------------------------
// Delete
// ---------------------------------------------------------------------------

#[test]
fn lib_compat_003_a_claim_holding_one_preserved_element_is_not_empty() {
    let input = package(&claim("c1", "", "<v:note><v:inner/><v:inner/></v:note>"));
    let mut doc = read("a Claim holding one preserved element", &input);
    let error = refuse(
        "DeleteElement c1 under RejectIfNonEmpty",
        &mut doc,
        DeleteElement::new("c1"),
    );
    assert!(
        matches!(&error, EditError::NotEmpty { element, contained: 1, .. } if element.as_str() == "c1"),
        "{ROW}: a preserved element (with its subtree) counts once: {error:?}"
    );
    assert_eq!(error.code(), Code::CMD_PACKAGE_NOT_EMPTY, "{ROW}: the code");
    let mutation = apply(
        "DeleteElement c1, recursively",
        &mut doc,
        DeleteElement::new("c1").with_package_policy(DeleteRecursively),
    );
    assert_eq!(
        mutation.deleted_ids(),
        ids(&["c1"]),
        "{ROW}: the deleted ids are elements only"
    );
    assert!(
        !write("after the delete", &doc).contains("v:note"),
        "{ROW}: deleting the holder removes its preserved content"
    );
}

#[test]
fn lib_compat_003_preserved_attributes_or_text_alone_do_not_make_an_element_non_empty() {
    let cases = [
        ("a preserved attribute", claim("c1", " v:w=\"1\"", "")),
        ("preserved text", claim("c1", "", "stray text")),
        (
            "a preserved attribute and text",
            claim("c1", " v:w=\"1\" isCounter=\"true\"", "stray"),
        ),
    ];
    for (what, body) in cases {
        let mut doc = read(what, &package(&body));
        assert!(
            doc.get("c1")
                .is_some_and(SacmElement::has_preserved_content),
            "{ROW}: {what}: setup: c1 holds preserved content"
        );
        let mutation = apply(what, &mut doc, DeleteElement::new("c1"));
        assert_eq!(
            mutation.deleted_ids(),
            ids(&["c1"]),
            "{ROW}: {what}: the delete under RejectIfNonEmpty"
        );
    }
}

#[test]
fn lib_compat_003_not_empty_counts_each_preserved_child_element_in_the_subtree() {
    // p holds: one preserved element, c1 (holding one preserved element with
    // a nested one), and c2 holding a Note that holds one preserved element.
    // Counted: c1, c2, and the three preserved elements; the Note and the
    // nested preserved element do not count.
    let input = package(
        &[
            "<v:first/>".to_owned(),
            claim("c1", " v:w=\"1\"", "<v:second><v:nested/></v:second>"),
            claim(
                "c2",
                "",
                "<note xmi:id=\"n1\"><content><value content=\"n\"/></content><v:third/></note>",
            ),
        ]
        .concat(),
    );
    let mut doc = read("preserved elements at every depth", &input);
    let error = refuse(
        "DeleteElement p under RejectIfNonEmpty",
        &mut doc,
        DeleteElement::new("p"),
    );
    assert!(
        matches!(&error, EditError::NotEmpty { contained: 5, .. }),
        "{ROW}: two Claims and three preserved elements: {error:?}"
    );
}

/// c1 holds the preserved `a1` and `a2` in package p1 of `case`; c2 in p1
/// cites a1, and c3 in package p2 has a1 and a2 as its metaClaims.
fn referenced_holder() -> String {
    case(&format!(
        "{}{}",
        argument_package(
            "p1",
            &format!(
                "{}{}",
                claim(
                    "c1",
                    "",
                    "<v:note xmi:id=\"a1\"><v:inner xmi:id=\"a2\"/></v:note>"
                ),
                claim("c2", " isCitation=\"true\" citedElement=\"a1\"", "")
            )
        ),
        argument_package("p2", &claim("c3", " metaClaim=\"a1 a2\"", ""))
    ))
}

#[test]
fn lib_compat_003_deleting_the_holder_of_a_referenced_preserved_id_is_referenced() {
    let mut doc = read("a referenced preserved id", &referenced_holder());
    let error = refuse(
        "DeleteElement c1, recursively, in the same package",
        &mut doc,
        DeleteElement::new("c1")
            .with_package_policy(DeleteRecursively)
            .with_cross_package_policy(ApplyReferencePolicy),
    );
    assert!(
        matches!(
            &error,
            EditError::Referenced { holder, end: "citedElement", id, .. }
                if holder.as_str() == "c2" && id.as_str() == "a1"
        ),
        "{ROW}: the first dangling reference is c2's citedElement a1: {error:?}"
    );
    assert_eq!(error.code(), Code::CMD_DELETE_REFERENCED, "{ROW}: the code");
    // c2's same-package reference comes first in dangling order, so the
    // reference policy must handle it for the cross-package one to decide.
    let error = refuse(
        "DeleteElement c1, recursively, scrubbing, under the default cross-package policy",
        &mut doc,
        DeleteElement::new("c1")
            .with_package_policy(DeleteRecursively)
            .with_reference_policy(ScrubReferences),
    );
    assert!(
        matches!(
            &error,
            EditError::CrossPackageReference { holder, end: "metaClaim", id, .. }
                if holder.as_str() == "c3" && id.as_str() == "a1"
        ),
        "{ROW}: c3 in p2 names a1, held in p1: {error:?}"
    );
    assert_eq!(
        error.code(),
        Code::CMD_EXTERNAL_REFERENCES,
        "{ROW}: the code"
    );
}

#[test]
fn lib_compat_003_preview_lists_references_to_preserved_ids_as_dangling() {
    let doc = read("a referenced preserved id", &referenced_holder());
    let delete = DeleteElement::new("c1")
        .with_package_policy(DeleteRecursively)
        .with_reference_policy(ScrubReferences)
        .with_cross_package_policy(ApplyReferencePolicy);
    let preview = edit::preview(&doc, &delete.into())
        .unwrap_or_else(|error| panic!("{ROW}: the preview was refused: {error:?}"));
    let dangling: Vec<(&str, ReferenceEnd, &str, bool)> = preview
        .dangling_references()
        .iter()
        .map(|d| {
            (
                d.holder().as_str(),
                d.end(),
                d.id().as_str(),
                d.crosses_package(),
            )
        })
        .collect();
    assert_eq!(
        dangling,
        [
            ("c2", ReferenceEnd::CitedElement, "a1", false),
            ("c3", ReferenceEnd::MetaClaim, "a1", true),
            ("c3", ReferenceEnd::MetaClaim, "a2", true),
        ],
        "{ROW}: every typed reference to a freed preserved id, crossing a package from the \
         holder's package"
    );
    assert_eq!(
        preview.mutation().deleted_ids(),
        ids(&["c1"]),
        "{ROW}: the preview's deleted ids are elements only"
    );
}

#[test]
fn lib_compat_003_scrub_references_frees_the_preserved_ids_for_with_id() {
    let mut doc = read("a referenced preserved id", &referenced_holder());
    let mutation = apply(
        "DeleteElement c1 under ScrubReferences",
        &mut doc,
        DeleteElement::new("c1")
            .with_package_policy(DeleteRecursively)
            .with_reference_policy(ScrubReferences)
            .with_cross_package_policy(ApplyReferencePolicy),
    );
    assert_eq!(
        mutation.deleted_ids(),
        ids(&["c1"]),
        "{ROW}: the deleted ids are elements only, not the preserved a1 and a2"
    );
    let c2 = doc.get("c2").expect("c2 survives");
    let c3 = doc.get("c3").expect("c3 survives");
    assert!(
        c2.cited_element().is_none() && c3.meta_claims().is_empty(),
        "{ROW}: the references to the freed ids are scrubbed"
    );
    let changed: BTreeSet<&str> = mutation
        .changed_ids()
        .iter()
        .map(ElementId::as_str)
        .collect();
    assert!(
        changed.contains("c2") && changed.contains("c3"),
        "{ROW}: the scrubbed holders are changed: {changed:?}"
    );
    assert!(
        validation::validate(&doc)
            .iter()
            .all(|f| !f.code().as_str().starts_with("SACM-REF-")),
        "{ROW}: no reference finding is left"
    );
    for freed in ["a1", "a2"] {
        apply(
            &format!("with_id({freed:?}) after the delete"),
            &mut doc,
            CreateClaim::new("p1", named("again")).with_id(freed),
        );
    }
}

// ---------------------------------------------------------------------------
// Positions under edits
// ---------------------------------------------------------------------------

#[test]
fn lib_compat_003_deleting_an_earlier_child_shifts_a_preserved_child_down() {
    let input = package(&format!(
        "{}{}<v:x/>{}",
        claim("c1", "", ""),
        claim("c2", "", ""),
        claim("c3", "", "")
    ));
    let mut doc = read("a preserved child before c3", &input);
    apply("DeleteElement c1", &mut doc, DeleteElement::new("c1"));
    let out = write("after deleting c1", &doc);
    assert_written_in_order(
        "a preserved child after deleting an earlier child",
        &out,
        &[&id_attribute("c2"), "<v:x", &id_attribute("c3")],
    );
}

#[test]
fn lib_compat_003_deleting_a_later_child_keeps_a_preserved_child_in_place() {
    let input = package(&format!(
        "{}<v:x/>{}{}",
        claim("c1", "", ""),
        claim("c2", "", ""),
        claim("c3", "", "")
    ));
    let mut doc = read("a preserved child before c2", &input);
    apply("DeleteElement c2", &mut doc, DeleteElement::new("c2"));
    let out = write("after deleting c2", &doc);
    assert_written_in_order(
        "a preserved child after deleting a later child",
        &out,
        &[&id_attribute("c1"), "<v:x", &id_attribute("c3")],
    );
}

/// The ids of the elements in both `before` and `after` for which
/// `semantic_compare(before, after)` reports a difference of the element
/// itself, in the pre-order of `after` (the LIB-EDIT-003 definition of
/// `changed_ids`).
fn compared_changed_ids(before: &Document, after: &Document) -> Vec<ElementId> {
    let differences = semantic_compare(before, after);
    let reported: BTreeSet<&ElementId> = differences
        .iter()
        .filter(|d| {
            !matches!(
                d,
                Difference::LeftOnly { .. } | Difference::RightOnly { .. }
            )
        })
        .filter_map(Difference::element_id)
        .collect();
    after
        .elements()
        .map(SacmElement::id)
        .filter(|id| reported.contains(id) && before.contains(id.as_str()))
        .cloned()
        .collect()
}

/// p holds a, a preserved child, then b.
fn around_a_preserved_child() -> String {
    package(&format!(
        "{}<v:x/>{}",
        claim("a", "", ""),
        claim("b", "", "")
    ))
}

/// Deletes `target` from [`around_a_preserved_child`] with `apply`,
/// `preview` and `apply_all`, asserts that each reports the changed ids
/// `semantic_compare` gives, and returns them.
fn delete_changed_ids(target: &str) -> Vec<ElementId> {
    let what = format!("DeleteElement {target}");
    let before = read(&what, &around_a_preserved_child());
    let operation = Operation::from(DeleteElement::new(target));

    let mut applied = before.clone();
    let mutation = apply(&what, &mut applied, operation.clone());
    let expected = compared_changed_ids(&before, &applied);
    assert_eq!(
        mutation.changed_ids(),
        expected,
        "{ROW}: {what}: apply's changed ids are those semantic_compare reports"
    );

    let preview = edit::preview(&before, &operation)
        .unwrap_or_else(|error| panic!("{ROW}: {what}: the preview was refused: {error:?}"));
    assert_eq!(
        preview.mutation().changed_ids(),
        expected,
        "{ROW}: {what}: the preview's changed ids agree"
    );

    let mut batched = before.clone();
    let batch = edit::apply_all(&mut batched, [operation])
        .unwrap_or_else(|error| panic!("{ROW}: {what}: apply_all was refused: {error:?}"));
    assert_eq!(
        batch.changed_ids(),
        expected,
        "{ROW}: {what}: apply_all's net changed ids agree"
    );
    assert_eq!(
        batch.mutations().first().map(|m| m.changed_ids().to_vec()),
        Some(expected.clone()),
        "{ROW}: {what}: the batch's own mutation agrees"
    );
    assert!(
        batched == applied,
        "{ROW}: {what}: apply and apply_all give the same document"
    );
    expected
}

#[test]
fn lib_compat_003_a_delete_that_moves_a_preserved_child_reports_its_parent_changed() {
    // Deleting a, before the preserved child, shifts its index: p's
    // preserved content differs, so p is changed.
    let changed = delete_changed_ids("a");
    assert_eq!(
        changed,
        ids(&["p"]),
        "{ROW}: deleting a child before a preserved child changes the parent"
    );
    // Control: deleting b, after it, leaves its index, so p is not changed.
    let changed = delete_changed_ids("b");
    assert!(
        changed.is_empty(),
        "{ROW}: deleting a child after a preserved child changes nothing else: {changed:?}"
    );
}

#[test]
fn lib_compat_003_reorder_keeps_a_preserved_child_s_index() {
    let input = package(&format!(
        "{}<v:x/>{}",
        claim("c1", "", ""),
        claim("c2", "", "")
    ));
    let mut doc = read("a preserved child between c1 and c2", &input);
    apply(
        "ReorderElements p [c2, c1]",
        &mut doc,
        ReorderElements::new("p", ["c2", "c1"]),
    );
    let out = write("after the reorder", &doc);
    assert_written_in_order(
        "a preserved child after a reorder",
        &out,
        &[&id_attribute("c2"), "<v:x", &id_attribute("c1")],
    );
}

#[test]
fn lib_compat_003_a_create_appends_after_a_trailing_preserved_child() {
    let input = package(&format!("{}<v:x/>", claim("c1", "", "")));
    let mut doc = read("a preserved child after c1", &input);
    apply(
        "CreateClaim c2",
        &mut doc,
        CreateClaim::new("p", named("c2")).with_id("c2"),
    );
    let out = write("after the create", &doc);
    assert_written_in_order(
        "a preserved child before a created child",
        &out,
        &[&id_attribute("c1"), "<v:x", &id_attribute("c2")],
    );
}

// ---------------------------------------------------------------------------
// Clone, and every other edit
// ---------------------------------------------------------------------------

#[test]
fn lib_compat_003_clone_copies_preserved_content() {
    let input = package(&format!(
        "{}{}",
        claim("c1", " v:w=\"1\"", "<v:note xmi:id=\"a1\">kept</v:note>"),
        claim("c2", "", "")
    ));
    let doc = read("preserved content to clone", &input);
    let mut copy = doc.clone();
    assert!(
        copy == doc && semantic_compare(&doc, &copy).is_empty(),
        "{ROW}: a clone is equal"
    );
    assert!(
        copy.get("c1")
            .is_some_and(SacmElement::has_preserved_content),
        "{ROW}: a clone's holder holds preserved content"
    );
    let original_out = write("the original", &doc);
    assert_eq!(
        write("the clone", &copy),
        original_out,
        "{ROW}: a clone writes the same preserved content"
    );
    assert!(
        original_out.contains("kept") && original_out.contains("v:w=\"1\""),
        "{ROW}: setup: the preserved content is written\n{original_out}"
    );
    apply(
        "DeleteElement c1 in the clone",
        &mut copy,
        DeleteElement::new("c1").with_package_policy(DeleteRecursively),
    );
    assert_eq!(
        write("the original after editing the clone", &doc),
        original_out,
        "{ROW}: editing a clone leaves the original's preserved content"
    );
    assert!(
        doc.get("c1")
            .is_some_and(SacmElement::has_preserved_content),
        "{ROW}: the original still holds it"
    );
}

/// p holds c1, a preserved child, c2 and c3; c1 holds a preserved
/// attribute, element and text and has c2 as its metaClaim. Without
/// `preserved`, the same document with none of it.
fn edit_document(preserved: bool) -> String {
    let (attribute, element, text, child) = if preserved {
        (
            " v:w=\"1\"",
            "<v:note xmi:id=\"a1\"><v:inner/></v:note>",
            "stray",
            "<v:x/>",
        )
    } else {
        ("", "", "", "")
    };
    package(&format!(
        "{}{child}{}{}",
        claim(
            "c1",
            &format!(" metaClaim=\"c2\"{attribute}"),
            &format!("{element}{text}")
        ),
        claim("c2", "", ""),
        claim("c3", "", "")
    ))
}

/// Asserts that `left` and `right` differ exactly by `expected`, in any
/// order.
fn assert_differ_by(what: &str, left: &Document, right: &Document, expected: &[Difference]) {
    let found = semantic_compare(left, right);
    assert!(
        found.len() == expected.len() && expected.iter().all(|d| found.contains(d)),
        "{ROW}: {what}: expected exactly {expected:?}, got {found:?}"
    );
}

#[test]
fn lib_compat_003_no_other_edit_changes_or_refuses_because_of_preserved_content() {
    let with = read("with preserved content", &edit_document(true));
    let without = read("without preserved content", &edit_document(false));
    let preserved = [
        Difference::PreservedContent { id: id("p") },
        Difference::PreservedContent { id: id("c1") },
    ];
    assert_differ_by("setup", &with, &without, &preserved);
    let note = LangString::new("n");
    let operations: Vec<(&str, Operation)> = vec![
        ("SetName c1", SetName::new("c1", named("renamed")).into()),
        ("SetGid c1", SetGid::new("c1", "g1").into()),
        (
            "SetFlag c1 isAbstract",
            SetFlag::new("c1", Flag::IsAbstract, true).into(),
        ),
        (
            "SetFlag c1 isCitation",
            SetFlag::new("c1", Flag::IsCitation, true).into(),
        ),
        (
            "SetReferences c1 citedElement",
            SetReferences::new("c1", ReferenceEnd::CitedElement, ["c3"]).into(),
        ),
        (
            "AddReference c1 metaClaim",
            AddReference::new("c1", ReferenceEnd::MetaClaim, "c3").into(),
        ),
        (
            "RemoveReference c1 metaClaim",
            RemoveReference::new("c1", ReferenceEnd::MetaClaim, "c2").into(),
        ),
        ("CreateNote in c1", CreateNote::new("c1", note).into()),
        (
            "CreateClaim in p",
            CreateClaim::new("p", named("c4")).with_id("c4").into(),
        ),
        (
            "CreateAssertedInference in p",
            CreateAssertedInference::new("p", named("i"), ["c3"], "c1").into(),
        ),
        (
            "ReorderElements p",
            ReorderElements::new("p", ["c3", "c2", "c1"]).into(),
        ),
        ("DeleteElement c3", DeleteElement::new("c3").into()),
        (
            "DeleteElement c2 (referenced)",
            DeleteElement::new("c2").into(),
        ),
    ];
    for (what, operation) in operations {
        let mut left = with.clone();
        let mut right = without.clone();
        let on_left = edit::apply(&mut left, operation.clone());
        let on_right = edit::apply(&mut right, operation);
        match (&on_left, &on_right) {
            (Ok(l), Ok(r)) => assert_eq!(
                (l.created_ids(), l.changed_ids(), l.deleted_ids()),
                (r.created_ids(), r.changed_ids(), r.deleted_ids()),
                "{ROW}: {what}: the mutations differ"
            ),
            (Err(l), Err(r)) => assert_eq!(l, r, "{ROW}: {what}: the refusals differ"),
            _ => panic!("{ROW}: {what}: with preserved content {on_left:?}, without {on_right:?}"),
        }
        assert_differ_by(what, &left, &right, &preserved);
    }
}

// ---------------------------------------------------------------------------
// RepeatedValue, and content with nowhere to be kept
// ---------------------------------------------------------------------------

#[test]
fn lib_compat_003_a_preserved_part_is_never_a_repeated_value_occurrence() {
    let cases = [
        (
            "a name and a namespaced name",
            claim("c1", "", "<v:name content=\"other\"/>"),
        ),
        (
            "isCounter, which a Claim lacks, as an attribute and an element",
            claim("c1", " isCounter=\"true\"", "<isCounter>true</isCounter>"),
        ),
        (
            "isCitation as an attribute and a namespaced element",
            claim(
                "c1",
                " isCitation=\"false\"",
                "<v:isCitation>true</v:isCitation>",
            ),
        ),
        (
            "a defective name before the name",
            "<argumentElement xmi:type=\"sacm:Claim\" xmi:id=\"c1\">\
             <name content=\"bad\" v:w=\"1\"/><name content=\"c1\"/></argumentElement>"
                .to_owned(),
        ),
        (
            "metaClaim as an attribute and a defective reference element",
            format!(
                "{}{}",
                claim(
                    "c1",
                    " metaClaim=\"c2\"",
                    "<metaClaim xmi:idref=\"c2\" v:w=\"1\"/>"
                ),
                claim("c2", "", "")
            ),
        ),
    ];
    for (what, body) in cases {
        for mode in MODES {
            let input = package(&body);
            let result = xmi::read(input.as_bytes(), &ReadOptions::default().with_mode(mode));
            match result {
                Ok(output) => {
                    let c1 = output.document().get("c1");
                    assert!(
                        c1.is_some_and(
                            |c1| c1.has_preserved_content() && c1.name() == Some(&named("c1"))
                        ),
                        "{ROW}: {what}: in {mode:?} mode c1 keeps its name and holds the \
                         preserved part"
                    );
                    assert!(
                        output
                            .diagnostics()
                            .iter()
                            .all(|f| f.code() == Code::XMI_UNKNOWN_CONTENT),
                        "{ROW}: {what}: in {mode:?} mode: {:?}",
                        output.diagnostics()
                    );
                }
                Err(error) => {
                    panic!("{ROW}: {what}: a {mode:?} read refused it: {error:?} ({error})")
                }
            }
        }
    }
}

/// Asserts that `input` is `Unsupported` in both modes, labelled a defect
/// in the input.
fn assert_defect(what: &str, input: &str) {
    for mode in MODES {
        let Err(error) = xmi::read(input.as_bytes(), &ReadOptions::default().with_mode(mode))
        else {
            panic!("{ROW}: {what}: a {mode:?} read accepted it");
        };
        let ReadError::Unsupported { reason, .. } = &error else {
            panic!("{ROW}: {what}: a {mode:?} read refused it as {error:?}, not as Unsupported");
        };
        assert!(
            reason.ends_with("(a defect in the input)"),
            "{ROW}: {what}: in {mode:?} mode the reason {reason:?} is not labelled a defect in \
             the input"
        );
    }
}

#[test]
fn lib_compat_003_content_on_the_xmi_wrapper_is_unsupported_as_a_defect() {
    let wrapper = |attributes: &str, body: &str| {
        format!(
            "<xmi:XMI xmlns:xmi=\"{XMI_NS}\" xmlns:sacm=\"{SACM_NS}\" xmlns:v=\"urn:example:vendor\"{attributes}>{body}\
             <sacm:ArgumentPackage xmi:id=\"p\"><name content=\"p\"/></sacm:ArgumentPackage></xmi:XMI>"
        )
    };
    assert_defect("an attribute on xmi:XMI", &wrapper(" v:w=\"1\"", ""));
    assert_defect(
        "an unqualified attribute on xmi:XMI",
        &wrapper(" w=\"1\"", ""),
    );
    assert_defect("text in xmi:XMI", &wrapper("", "stray"));
}

#[test]
fn lib_compat_003_a_preserved_attribute_binding_sacm_or_xmi_elsewhere_is_unsupported_as_a_defect() {
    assert_defect(
        "sacm:extra with sacm bound to another URI",
        &package(&format!(
            "<argumentElement xmlns:s=\"{SACM_NS}\" xmlns:sacm=\"urn:example:other\" \
             xmi:type=\"s:Claim\" xmi:id=\"c1\" sacm:extra=\"1\"><name content=\"c1\"/></argumentElement>"
        )),
    );
    assert_defect(
        "xmi:extra with xmi bound to another URI",
        &package(&format!(
            "<argumentElement xmlns:x=\"{XMI_NS}\" xmlns:xmi=\"urn:example:other\" \
             x:type=\"sacm:Claim\" x:id=\"c1\" xmi:extra=\"1\"><name content=\"c1\"/></argumentElement>"
        )),
    );
}
