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

/// A Claim c1 holding the preserved `a1`, and one element per reference
/// end naming a1: citedElement, abstractForm and metaClaim on Claims, the
/// source and reasoning of an inference, a structure, a
/// referencedArtifactElement and a group member.
fn references_to_a_preserved_id() -> String {
    package(
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
    )
}

/// The SACM-REF-003 findings `references_to_a_preserved_id` gives, as
/// (affected, clause, message), sorted.
fn expected_ref_003() -> Vec<(Vec<ElementId>, Option<String>, String)> {
    let message = |kind: &str, referrer: &str, end: &str, declared: &str| {
        format!(
            "{kind} {referrer:?} has the {end} \"a1\", which names an element the document holds \
             only as preserved content, so its type is not checked (the end's type is {declared})"
        )
    };
    let mut expected: Vec<(Vec<ElementId>, Option<String>, String)> = [
        ("Claim", "c2", "citedElement", "8.2", "SACMElement"),
        ("Claim", "c3", "abstractForm", "8.2", "SACMElement"),
        ("Claim", "c4", "metaClaim", "11.10", "Claim"),
        (
            "AssertedInference",
            "i1",
            "source",
            "11.13",
            "ArgumentAsset",
        ),
        (
            "AssertedInference",
            "i1",
            "reasoning",
            "11.13",
            "ArgumentReasoning",
        ),
        (
            "ArgumentReasoning",
            "r1",
            "structure",
            "11.12",
            "ArgumentPackage",
        ),
        (
            "ArtifactReference",
            "ar1",
            "referencedArtifactElement",
            "11.9",
            "ArtifactElement",
        ),
        (
            "ArgumentGroup",
            "g1",
            "argumentElement",
            "11.2",
            "ArgumentationElement",
        ),
    ]
    .iter()
    .map(|(kind, referrer, end, clause, declared)| {
        (
            ids(&[referrer, "a1"]),
            Some((*clause).to_owned()),
            message(kind, referrer, end, declared),
        )
    })
    .collect();
    expected.sort();
    expected
}

#[test]
fn lib_compat_003_a_typed_reference_to_a_preserved_id_is_ref_003_on_every_end() {
    let input = references_to_a_preserved_id();
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
    let mut got: Vec<(Vec<ElementId>, Option<String>, String)> = preserved
        .iter()
        .map(|f| {
            let (affected, clause) = affected_and_clause(f);
            (affected, clause, f.message().to_owned())
        })
        .collect();
    got.sort();
    let expected = expected_ref_003();
    for (_, _, text) in &got {
        assert!(
            !text.contains("  "),
            "{ROW}: a SACM-REF-003 message holds a run of spaces: {text:?}"
        );
    }
    assert_eq!(
        got, expected,
        "{ROW}: one SACM-REF-003 per end naming a preserved id, affecting [referrer, id], with \
         the end's clause and the exact message"
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

/// The SACM-REF-* findings of `doc`, as (code, affected), sorted.
fn reference_findings(doc: &Document) -> Vec<(Code, Vec<ElementId>)> {
    let mut found: Vec<(Code, Vec<ElementId>)> = validation::validate(doc)
        .iter()
        .filter(|f| f.code().as_str().starts_with("SACM-REF-"))
        .map(|f| (f.code(), f.affected().to_vec()))
        .collect();
    found.sort_by(|left, right| (left.0.as_str(), &left.1).cmp(&(right.0.as_str(), &right.1)));
    found
}

#[test]
fn lib_compat_003_a_reference_to_a_name_id_stays_ref_001_after_a_delete() {
    // SACM-REF-003 is for an id held only in preserved content; the
    // `xmi:id` of a name identifies no element, so a reference to it is
    // SACM-REF-001 (SACM23-VAL-003). A delete elsewhere must not move an id
    // from one class to the other: n1 stays a name's id, a1 a preserved one.
    let input = package(
        &[
            "<argumentElement xmi:type=\"sacm:Claim\" xmi:id=\"c1\">\
             <name xmi:id=\"n1\" content=\"c1\"/><v:note xmi:id=\"a1\"/></argumentElement>"
                .to_owned(),
            claim("c2", " isCitation=\"true\" citedElement=\"n1\"", ""),
            claim("c3", "", ""),
            claim("c4", " metaClaim=\"a1\"", ""),
        ]
        .concat(),
    );
    let mut doc = read("a reference to a name's id", &input);
    let expected = [
        (Code::REF_DANGLING, ids(&["c2", "n1"])),
        (Code::REF_PRESERVED_TARGET, ids(&["c4", "a1"])),
    ];
    assert_eq!(
        reference_findings(&doc),
        expected,
        "{ROW}: setup: c2's citedElement n1 is SACM-REF-001, c4's metaClaim a1 SACM-REF-003"
    );
    let mutation = apply("DeleteElement c3", &mut doc, DeleteElement::new("c3"));
    assert_eq!(
        (mutation.deleted_ids(), mutation.changed_ids()),
        (&ids(&["c3"])[..], &[][..]),
        "{ROW}: the delete of c3 deletes c3 alone and changes nothing else"
    );
    assert_eq!(
        reference_findings(&doc),
        expected,
        "{ROW}: after deleting c3, a reference to the name's id n1 is still SACM-REF-001, not \
         SACM-REF-003, and one to the preserved a1 still SACM-REF-003"
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
// An end read empty, then set by an edit
// ---------------------------------------------------------------------------

/// p holding the Claims c1 and c2, an argument reasoning r1, an inference
/// i1, an artifact reference ar1 and a group g1, with `holder`'s attribute
/// `end` written empty instead of as usual.
fn empty_end_document(holder: &str, end: &str) -> String {
    let extra = |id: &str, own: &[(&str, &str)]| -> String {
        let mut attributes = String::new();
        for (name, value) in own {
            let value = if id == holder && *name == end {
                ""
            } else {
                value
            };
            attributes.push_str(&[" ", name, "=\"", value, "\""].concat());
        }
        if id == holder && !own.iter().any(|(name, _)| *name == end) {
            attributes.push_str(&[" ", end, "=\"\""].concat());
        }
        attributes
    };
    package(
        &[
            claim("c1", &extra("c1", &[]), ""),
            claim("c2", "", ""),
            element("ArgumentReasoning", "r1", &extra("r1", &[]), ""),
            element(
                "AssertedInference",
                "i1",
                &extra("i1", &[("source", "c1"), ("target", "c2")]),
                "",
            ),
            element("ArtifactReference", "ar1", &extra("ar1", &[]), ""),
            element("ArgumentGroup", "g1", &extra("g1", &[]), ""),
        ]
        .concat(),
    )
}

#[test]
fn lib_compat_003_setting_an_end_read_empty_replaces_the_preserved_attribute() {
    // An empty reference attribute is preserved (SACM-XMI-003). The edit
    // that sets the end replaces the preserved attribute of the same name
    // (the lead's decision), so the write holds the attribute once and reads
    // back as the edited document.
    let cases: Vec<(&str, &str, Vec<Operation>)> = vec![
        (
            "c1",
            "metaClaim",
            vec![AddReference::new("c1", ReferenceEnd::MetaClaim, "c2").into()],
        ),
        (
            "c1",
            "citedElement",
            vec![
                SetFlag::new("c1", Flag::IsCitation, true).into(),
                SetReferences::new("c1", ReferenceEnd::CitedElement, ["c2"]).into(),
            ],
        ),
        (
            "c1",
            "abstractForm",
            vec![SetReferences::new("c1", ReferenceEnd::AbstractForm, ["c2"]).into()],
        ),
        (
            "r1",
            "structure",
            vec![SetReferences::new("r1", ReferenceEnd::Structure, ["p"]).into()],
        ),
        (
            "i1",
            "source",
            vec![AddReference::new("i1", ReferenceEnd::Source, "c1").into()],
        ),
        (
            "i1",
            "target",
            vec![SetReferences::new("i1", ReferenceEnd::Target, ["c2"]).into()],
        ),
        (
            "i1",
            "reasoning",
            vec![SetReferences::new("i1", ReferenceEnd::Reasoning, ["r1"]).into()],
        ),
        (
            "ar1",
            "referencedArtifactElement",
            vec![AddReference::new("ar1", ReferenceEnd::ReferencedArtifactElement, "c1").into()],
        ),
        (
            "g1",
            "argumentElement",
            vec![AddReference::new("g1", ReferenceEnd::GroupMember, "c1").into()],
        ),
    ];
    for (holder, end, operations) in cases {
        let what = format!("{holder}'s {end}=\"\", then set by an edit");
        let input = empty_end_document(holder, end);
        let mut doc = read(&what, &input);
        assert!(
            doc.get(holder)
                .is_some_and(SacmElement::has_preserved_content),
            "{ROW}: {what}: setup: the empty {end} is preserved on {holder}"
        );
        for operation in operations {
            apply(&what, &mut doc, operation);
        }
        assert!(
            doc.get(holder).is_some_and(|e| !e.has_preserved_content()),
            "{ROW}: {what}: the edit replaced the preserved {end}, so {holder} holds none"
        );
        let out = write(&what, &doc);
        let attribute = format!(" {end}=\"");
        assert_eq!(
            out.matches(&attribute).count(),
            1,
            "{ROW}: {what}: the output holds {end} once\n{out}"
        );
        let again = match xmi::read(out.as_bytes(), &ReadOptions::default()) {
            Ok(output) => {
                assert!(
                    output.diagnostics().is_empty(),
                    "{ROW}: {what}: the output reads with {:?}",
                    output.diagnostics()
                );
                output.into_document()
            }
            Err(error) => panic!("{ROW}: {what}: the output does not read: {error:?}\n{out}"),
        };
        assert!(
            again == doc && semantic_compare(&doc, &again).is_empty(),
            "{ROW}: {what}: the output reads as the edited document: {:?}",
            semantic_compare(&doc, &again)
        );
        assert!(
            again
                .get(holder)
                .is_some_and(|e| !e.has_preserved_content()),
            "{ROW}: {what}: no preserved content is read back on {holder}"
        );
    }
}

/// p holding the Claim c1 with `attributes` and `body` (and no name when
/// `named` is false), then the Claim c2.
fn metaclaim_document(attributes: &str, body: &str, named: bool) -> String {
    let name = if named { "<name content=\"c1\"/>" } else { "" };
    package(&format!(
        "<argumentElement xmi:type=\"sacm:Claim\" xmi:id=\"c1\"{attributes}>{name}{body}</argumentElement>{}",
        claim("c2", "", "")
    ))
}

/// Reads `input`, applies `operation`, and returns the document before, the
/// one after, the mutation and the tolerant write after.
fn edited(
    what: &str,
    input: &str,
    operation: impl Into<Operation>,
) -> (Document, Mutation, String) {
    let mut doc = read(what, input);
    let mutation = apply(what, &mut doc, operation);
    let out = write(what, &doc);
    (doc, mutation, out)
}

/// Asserts that `out` reads back, in tolerant mode, as `doc`.
fn assert_reads_back(what: &str, doc: &Document, out: &str) {
    let again = read(&format!("{what}: the output"), out);
    assert!(
        again == *doc && semantic_compare(doc, &again).is_empty(),
        "{ROW}: {what}: the output reads as the edited document: {:?}",
        semantic_compare(doc, &again)
    );
}

fn holds_preserved(doc: &Document, id: &str) -> bool {
    doc.get(id).is_some_and(SacmElement::has_preserved_content)
}

#[test]
fn lib_compat_003_setting_the_ids_held_still_replaces_an_empty_preserved_end() {
    // c1 holds metaClaim [c2] as an element and metaClaim="" preserved.
    // Setting [c2] again sets the ids held, but replaces the preserved
    // attribute, so it changes c1.
    let what = "SetReferences c1 metaClaim [c2] beside metaClaim=\"\"";
    let input = metaclaim_document(" metaClaim=\"\"", "<metaClaim xmi:idref=\"c2\"/>", true);
    let (doc, mutation, out) = edited(
        what,
        &input,
        SetReferences::new("c1", ReferenceEnd::MetaClaim, ["c2"]),
    );
    assert_eq!(
        mutation.changed_ids(),
        ids(&["c1"]),
        "{ROW}: {what}: replacing the preserved attribute changes c1"
    );
    assert!(
        !holds_preserved(&doc, "c1"),
        "{ROW}: {what}: c1 holds no preserved content"
    );
    assert_eq!(
        out.matches(" metaClaim=\"").count(),
        1,
        "{ROW}: {what}: metaClaim is written once\n{out}"
    );
    assert_reads_back(what, &doc, &out);
}

#[test]
fn lib_compat_003_setting_an_end_to_no_ids_keeps_an_empty_preserved_end() {
    let what = "SetReferences c1 metaClaim [] with only metaClaim=\"\"";
    let input = metaclaim_document(" metaClaim=\"\"", "", true);
    let (doc, mutation, out) = edited(
        what,
        &input,
        SetReferences::clear("c1", ReferenceEnd::MetaClaim),
    );
    assert!(
        mutation.changed_ids().is_empty(),
        "{ROW}: {what}: setting no ids on an empty end changes nothing: {:?}",
        mutation.changed_ids()
    );
    assert!(
        holds_preserved(&doc, "c1"),
        "{ROW}: {what}: the preserved metaClaim=\"\" is kept"
    );
    assert_eq!(
        out.matches(" metaClaim=\"\"").count(),
        1,
        "{ROW}: {what}: the preserved attribute is written once\n{out}"
    );
}

#[test]
fn lib_compat_003_a_namespaced_attribute_of_an_end_s_name_is_not_that_end() {
    // v:metaClaim="" is preserved, but it is not the end metaClaim.
    let what = "metaClaim=\"c2\" beside a preserved v:metaClaim=\"\"";
    let input = metaclaim_document(" metaClaim=\"c2\" v:metaClaim=\"\"", "", true);
    let (doc, mutation, out) = edited(
        what,
        &input,
        SetReferences::new("c1", ReferenceEnd::MetaClaim, ["c2"]),
    );
    assert!(
        mutation.changed_ids().is_empty(),
        "{ROW}: {what}: setting the ids held changes nothing: {:?}",
        mutation.changed_ids()
    );
    assert!(
        out.contains(" metaClaim=\"c2\"")
            && out.contains("v:metaClaim=\"\"")
            && !out.contains("<metaClaim"),
        "{ROW}: {what}: the end keeps its attribute form beside the preserved one\n{out}"
    );
    assert!(
        holds_preserved(&doc, "c1"),
        "{ROW}: {what}: v:metaClaim stays preserved"
    );
    assert_reads_back(what, &doc, &out);
}

#[test]
fn lib_compat_003_setting_an_end_replaces_only_the_unqualified_attribute_of_its_name() {
    let what = "v:metaClaim=\"1\" before metaClaim=\"\", then SetReferences [c2]";
    let input = metaclaim_document(" v:metaClaim=\"1\" metaClaim=\"\"", "", true);
    let (doc, _, out) = edited(
        what,
        &input,
        SetReferences::new("c1", ReferenceEnd::MetaClaim, ["c2"]),
    );
    assert!(
        out.contains("v:metaClaim=\"1\""),
        "{ROW}: {what}: the namespaced attribute is kept\n{out}"
    );
    assert!(
        !out.contains(" metaClaim=\"\"") && out.matches(" metaClaim=\"").count() == 1,
        "{ROW}: {what}: the empty metaClaim is replaced by the end\n{out}"
    );
    assert_reads_back(what, &doc, &out);
}

#[test]
fn lib_compat_003_removing_the_last_id_keeps_an_empty_preserved_end() {
    let what = "RemoveReference c1 metaClaim c2 beside metaClaim=\"\"";
    let input = metaclaim_document(" metaClaim=\"\"", "<metaClaim xmi:idref=\"c2\"/>", true);
    let (doc, _, out) = edited(
        what,
        &input,
        RemoveReference::new("c1", ReferenceEnd::MetaClaim, "c2"),
    );
    assert!(
        holds_preserved(&doc, "c1"),
        "{ROW}: {what}: an end left with no id does not replace the preserved attribute"
    );
    assert_eq!(
        out.matches(" metaClaim=\"").count(),
        1,
        "{ROW}: {what}: the preserved metaClaim=\"\" is written once\n{out}"
    );
    assert!(
        out.contains(" metaClaim=\"\"") && !out.contains("<metaClaim"),
        "{ROW}: {what}: only the preserved attribute is left\n{out}"
    );
    assert_reads_back(what, &doc, &out);
}

#[test]
fn lib_compat_003_removing_one_of_two_ids_beside_an_empty_preserved_end_writes_the_end_once() {
    // c1 holds metaClaim [c2, c3] as elements and metaClaim="" preserved.
    // Removing c2 leaves c3 in the end, which replaces the preserved
    // attribute, so the end is written once and c1 changes.
    let what = "RemoveReference c1 metaClaim c2 of [c2, c3] beside metaClaim=\"\"";
    let input = package(&format!(
        "<argumentElement xmi:type=\"sacm:Claim\" xmi:id=\"c1\" metaClaim=\"\">\
         <name content=\"c1\"/><metaClaim xmi:idref=\"c2\"/><metaClaim xmi:idref=\"c3\"/>\
         </argumentElement>{}{}",
        claim("c2", "", ""),
        claim("c3", "", "")
    ));
    let before = read(what, &input);
    assert!(
        holds_preserved(&before, "c1")
            && before
                .get("c1")
                .is_some_and(|c1| c1.meta_claims() == ids(&["c2", "c3"]).as_slice()),
        "{ROW}: {what}: setup: c1 holds [c2, c3] and the preserved metaClaim=\"\""
    );
    let (doc, mutation, out) = edited(
        what,
        &input,
        RemoveReference::new("c1", ReferenceEnd::MetaClaim, "c2"),
    );
    assert_eq!(
        mutation.changed_ids(),
        ids(&["c1"]),
        "{ROW}: {what}: the remove changes c1 only"
    );
    assert!(
        doc.get("c1")
            .is_some_and(|c1| c1.meta_claims() == ids(&["c3"]).as_slice()),
        "{ROW}: {what}: c1's metaClaim is [c3]"
    );
    assert!(
        !holds_preserved(&doc, "c1"),
        "{ROW}: {what}: the end left with c3 replaces the preserved attribute, so c1 holds none"
    );
    let attributes = out.matches(" metaClaim=\"").count();
    let elements = out.matches("<metaClaim").count();
    assert_eq!(
        attributes + elements,
        1,
        "{ROW}: {what}: the end is written once ({attributes} attributes, {elements} elements)\n{out}"
    );
    assert!(
        !out.contains(" metaClaim=\"\""),
        "{ROW}: {what}: the empty metaClaim is not written\n{out}"
    );
    assert_reads_back(what, &doc, &out);
}

#[test]
fn lib_compat_003_an_unnamed_holder_writes_an_end_beside_its_preserved_attribute_as_an_element() {
    // c1 has no name: the reference element is what keeps its start tag
    // open.
    let what = "c1 with no name, metaClaim=\"\" and <metaClaim xmi:idref=\"c2\"/>";
    let doc = read(
        what,
        &metaclaim_document(" metaClaim=\"\"", "<metaClaim xmi:idref=\"c2\"/>", false),
    );
    let out = write(what, &doc);
    assert_eq!(
        out,
        concat!(
            "<?xml version=\"1.0\" encoding=\"UTF-8\"?>\n",
            "<sacm:ArgumentPackage xmlns:sacm=\"http://www.omg.org/spec/SACM/20220301\" ",
            "xmlns:xmi=\"http://www.omg.org/spec/XMI/20131001\" xmi:id=\"p\">\n",
            "  <name content=\"p\" />\n",
            "  <argumentElement xmi:type=\"sacm:Claim\" xmi:id=\"c1\" metaClaim=\"\">\n",
            "    <metaClaim xmi:idref=\"c2\" />\n",
            "  </argumentElement>\n",
            "  <argumentElement xmi:type=\"sacm:Claim\" xmi:id=\"c2\">\n",
            "    <name content=\"c2\" />\n",
            "  </argumentElement>\n",
            "</sacm:ArgumentPackage>\n",
        ),
        "{ROW}: {what}: the exact output"
    );
    assert_reads_back(what, &doc, &out);

    // With only the preserved attribute, the end is empty and nothing is
    // written as an element: the start tag closes itself.
    let what = "c1 with no name and only metaClaim=\"\"";
    let doc = read(what, &metaclaim_document(" metaClaim=\"\"", "", false));
    let out = write(what, &doc);
    assert!(
        out.contains("<argumentElement xmi:type=\"sacm:Claim\" xmi:id=\"c1\" metaClaim=\"\" />\n"),
        "{ROW}: {what}: c1 is written as one empty element\n{out}"
    );
    assert_reads_back(what, &doc, &out);
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

/// Asserts that `input` is `Unsupported` in strict mode, labelled a defect
/// in the input. A tolerant read keeps it, with SACM-XMI-003 (ADR 0035;
/// `document_preserved_content.rs`).
fn assert_defect(what: &str, input: &str) {
    {
        let mode = Mode::Strict;
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

/// A tolerant read keeps a preserved attribute whose prefix `sacm` or `xmi`
/// is bound to another URI, with one SACM-XMI-003 naming the new prefix: its
/// namespace URI and local name are kept, and the prefix is replaced by the
/// first `nsN` unbound at the holder (ADR 0033 amendment 1, ADR 0035).
#[test]
fn lib_compat_003_a_rebound_sacm_prefix_is_kept_under_ns1() {
    for (what, holder_attributes, renamed) in [
        (
            "sacm bound to another URI",
            format!(
                "xmlns:s=\"{SACM_NS}\" xmlns:sacm=\"urn:example:other\" xmi:type=\"s:Claim\" \
                 xmi:id=\"c1\" sacm:extra=\"1\""
            ),
            "ns1",
        ),
        (
            "xmi bound to another URI",
            format!(
                "xmlns:x=\"{XMI_NS}\" xmlns:xmi=\"urn:example:other\" x:type=\"sacm:Claim\" \
                 x:id=\"c1\" xmi:extra=\"1\""
            ),
            "ns1",
        ),
        (
            "sacm bound to another URI, and ns1 bound at the holder",
            format!(
                "xmlns:s=\"{SACM_NS}\" xmlns:sacm=\"urn:example:other\" xmlns:ns1=\"urn:taken\" \
                 xmi:type=\"s:Claim\" xmi:id=\"c1\" ns1:kept=\"2\" sacm:extra=\"1\""
            ),
            "ns2",
        ),
    ] {
        let input = package(&format!(
            "<argumentElement {holder_attributes}><name content=\"c1\"/></argumentElement>"
        ));
        let Err(strict) = xmi::read(
            input.as_bytes(),
            &ReadOptions::default().with_mode(Mode::Strict),
        ) else {
            panic!("{ROW}: {what}: setup: a strict read accepted it");
        };
        let output = match xmi::read(input.as_bytes(), &ReadOptions::default()) {
            Ok(output) => output,
            Err(error) => panic!("{ROW}: {what}: a tolerant read refused it: {error:?}"),
        };
        let at_extra: Vec<&Diagnostic> = output
            .diagnostics()
            .iter()
            .filter(|found| found.location() == strict.location())
            .collect();
        assert!(
            at_extra.len() == 1
                && at_extra[0].code() == Code::XMI_UNKNOWN_CONTENT
                && at_extra[0].severity() == Severity::Warning
                && at_extra[0].affected() == ids(&["c1"])
                && at_extra[0].message().contains(renamed),
            "{ROW}: {what}: not one SACM-XMI-003 Warning on c1 at the attribute naming \
             {renamed}: {:?}",
            output.diagnostics()
        );
        let doc = output.into_document();
        assert!(
            doc.get("c1")
                .is_some_and(SacmElement::has_preserved_content),
            "{ROW}: {what}: c1 does not hold the attribute"
        );
        let text = write(what, &doc);
        let Some(tag) = text.lines().find(|line| line.contains("xmi:id=\"c1\"")) else {
            panic!("{ROW}: {what}: c1 is not written\n{text}");
        };
        assert!(
            tag.contains(&format!("xmlns:{renamed}=\"urn:example:other\""))
                && tag.contains(&format!("{renamed}:extra=\"1\""))
                && !tag.contains("sacm:extra")
                && !tag.contains("xmi:extra"),
            "{ROW}: {what}: the attribute is not written as {renamed}:extra in urn:example:other: \
             {tag}"
        );
        let back = read(&format!("{what}, written"), &text);
        assert!(
            back == doc && semantic_compare(&doc, &back).is_empty(),
            "{ROW}: {what}: the output does not read back equal\n{text}"
        );
        assert_eq!(
            write(what, &back),
            text,
            "{ROW}: {what}: writing again is not byte-identical"
        );
    }
}
