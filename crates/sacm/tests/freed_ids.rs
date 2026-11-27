//! Tests for LIB-EDIT-026: edits never remove an id that is still named.
//!
//! XML Schema Part 1 §3.3.4 Validation Root Valid (ID/IDREF), clause 1:
//! "There must be no ID/IDREF binding in the item's [ID/IDREF table] whose
//! [binding] is the empty set." XMI 2.5.1 §7.6.2: `idref` "allows an XML
//! element to refer to another XML element within the same document using the
//! XML IDREF mechanism"; XMI.xsd types `target`, `container`, `addition` and
//! `replacement` `xsd:IDREFS`. SACM 2.3 §2.2: a tool must "import and export
//! XMI documents that conform with the SACM XML Schema". So:
//!
//! - **No edit removes an `xmi:id` that is still named.** `DeleteElement`
//!   (under every policy), `SetContent`, `SetKey`, `SetLocation` and
//!   `SetTaggedValue` refuse to remove the id of a name, `content`, `key`,
//!   `location` or `value`, or one in its XMI metadata, that a reference end
//!   of a remaining element names (`EditError::PartReference`), and to leave
//!   a token of XMI metadata, as a strict write checks it (SACM23-XMI-019),
//!   naming a removed id (`EditError::MetadataReference`).
//! - **No copy brings in metadata naming an id the target lacks**
//!   (`EditError::MetadataReference`, naming the copy).
//! - **Both are SACM-REF-001,** after every other check and before
//!   LIB-EDIT-025's text check; `check`, `preview`, `apply` and the batches
//!   agree, and a refusal changes nothing.
//! - **A reference to a deleted element stays the delete policies'.**
//! - **So a document that a strict read and a strict write accept writes
//!   strictly after any accepted edits,** while it has a root.
//!
//! The worked fixture is `sacm23/xmi/named-ids-valid.sacm.xmi`.

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use sacm::diagnostic::{Code, Severity};
use sacm::edit::{
    self, AddReference, BatchError, CopyElements, CreateClaim, CrossPackageReferencePolicy,
    DeleteElement, EditError, MoveElement, Mutation, Operation, PackageDeletePolicy,
    ReferenceDeletePolicy, RemoveReference, ReorderElements, SetContent, SetFlag, SetGid, SetKey,
    SetLocation, SetName, SetReferences, SetTaggedValue,
};
use sacm::model::{
    Document, ElementId, ElementKind, Flag, LangString, MultiLangString, ReferenceEnd, SacmElement,
};
use sacm::validation;
use sacm::xmi::{self, Mode, ReadOptions, WriteError, WriteOptions};

const ROW: &str = "LIB-EDIT-026";

/// The namespace declarations of a document element: `sacm` and `xmi`.
const NS: &str = concat!(
    " xmlns:sacm=\"http://www.omg.org/spec/SACM/20220301\"",
    " xmlns:xmi=\"http://www.omg.org/spec/XMI/20131001\"",
);

/// The worked fixture of the slice.
const FIXTURE: &str = concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/tests/data/sacm23/xmi/named-ids-valid.sacm.xmi"
);

/// The catalog of diagnostic codes, relative to the repository root.
const CATALOG: &str = "docs/sacm/diagnostics.md";

/// An id no element of any test document has.
const NOWHERE: &str = "nowhere";

// ---------------------------------------------------------------------------
// The designed API of LIB-EDIT-026: the two new variants
// ---------------------------------------------------------------------------

/// What a refusal of LIB-EDIT-026 names.
#[derive(Clone, Debug, PartialEq, Eq)]
enum Named {
    /// `EditError::PartReference`: the holder of the reference, its end and
    /// the id it names.
    Part {
        holder: String,
        end: &'static str,
        id: String,
    },
    /// `EditError::MetadataReference`: the element whose metadata holds the
    /// token (`None` for the document's own), the metadata element and
    /// attribute as written, and the id the token names.
    Metadata {
        holder: Option<String>,
        element: String,
        attribute: String,
        id: String,
    },
}

/// What `error` names, if it is a `PartReference` or a `MetadataReference`.
fn named(error: &EditError) -> Option<Named> {
    match error {
        EditError::PartReference {
            holder, end, id, ..
        } => Some(Named::Part {
            holder: holder.as_str().to_owned(),
            end,
            id: id.as_str().to_owned(),
        }),
        EditError::MetadataReference {
            holder,
            element,
            attribute,
            id,
            ..
        } => Some(Named::Metadata {
            holder: holder.as_ref().map(|holder| holder.as_str().to_owned()),
            element: element.clone(),
            attribute: attribute.clone(),
            id: id.as_str().to_owned(),
        }),
        _ => None,
    }
}

/// A `PartReference` naming `holder`, `end` and `id`.
fn part(holder: &str, end: &'static str, id: &str) -> Named {
    Named::Part {
        holder: holder.to_owned(),
        end,
        id: id.to_owned(),
    }
}

/// A `MetadataReference` naming `holder`, `element`, `attribute` and `id`.
fn meta(holder: Option<&str>, element: &str, attribute: &str, id: &str) -> Named {
    Named::Metadata {
        holder: holder.map(str::to_owned),
        element: element.to_owned(),
        attribute: attribute.to_owned(),
        id: id.to_owned(),
    }
}

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

fn en(content: &str) -> LangString {
    LangString::new(content).with_lang("en")
}

fn sv(content: &str) -> LangString {
    LangString::new(content).with_lang("sv")
}

/// The values `values`, in order.
fn multi(values: &[LangString]) -> MultiLangString {
    values.iter().cloned().collect()
}

/// `text` read in `mode`, which must succeed.
fn read_in(what: &str, mode: Mode, text: &str) -> Document {
    match xmi::read(text.as_bytes(), &ReadOptions::default().with_mode(mode)) {
        Ok(output) => output.into_document(),
        Err(error) => panic!("{ROW}: setup: {what} does not read ({mode:?}): {error}\n{text}"),
    }
}

/// `text`, which a strict read must accept without a diagnostic and a strict
/// write must accept, read back equal.
fn writable(what: &str, text: &str) -> Document {
    let output = match xmi::read(text.as_bytes(), &ReadOptions::default()) {
        Ok(output) => output,
        Err(error) => panic!("{ROW}: setup: {what} does not read strictly: {error}\n{text}"),
    };
    assert!(
        output.diagnostics().is_empty(),
        "{ROW}: setup: a strict read of {what} reports {:?}",
        output
            .diagnostics()
            .iter()
            .map(ToString::to_string)
            .collect::<Vec<_>>()
    );
    let doc = output.into_document();
    still_strict(&format!("setup: {what}"), &doc);
    doc
}

/// Asserts that a strict write accepts `doc` and that its output reads back
/// strictly equal.
fn still_strict(what: &str, doc: &Document) {
    let text = match xmi::write(doc, &WriteOptions::default()) {
        Ok(text) => text,
        Err(error) => panic!("{ROW}: {what}: a strict write refuses: {error} ({error:?})"),
    };
    let back = match xmi::read(text.as_bytes(), &ReadOptions::default()) {
        Ok(output) => output.into_document(),
        Err(error) => panic!("{ROW}: {what}: the strict output does not read back: {error}"),
    };
    assert!(
        back == *doc,
        "{ROW}: {what}: the strict output does not read back equal\n{text}"
    );
}

/// What both modes of `xmi::write` give for `doc`: the text, or the refusal.
fn written(doc: &Document) -> [Result<String, String>; 2] {
    [Mode::Tolerant, Mode::Strict].map(|mode| {
        xmi::write(doc, &WriteOptions::default().with_mode(mode)).map_err(|error| error.to_string())
    })
}

/// The fixture `named-ids-valid.sacm.xmi`, read. A strict read reports
/// nothing and a strict write accepts it, while `validate` reports the three
/// citations of part ids as SACM-REF-001 (SACM23-VAL-003).
fn fixture() -> Document {
    let text = std::fs::read_to_string(FIXTURE)
        .unwrap_or_else(|error| panic!("{ROW}: setup: read {FIXTURE}: {error}"));
    let doc = writable("named-ids-valid.sacm.xmi", &text);
    let findings = validation::validate(&doc);
    let found: Vec<(Code, Severity, Vec<&str>)> = findings
        .iter()
        .map(|finding| {
            (
                finding.code(),
                finding.severity(),
                finding.affected().iter().map(ElementId::as_str).collect(),
            )
        })
        .collect();
    let dangling = |holder, id| (Code::REF_DANGLING, Severity::Error, vec![holder, id]);
    assert_eq!(
        found,
        [
            dangling("c2", "d1v1"),
            dangling("c3", "c1n"),
            dangling("r2", "r1lv")
        ],
        "{ROW}: setup: validate does not report the fixture's citations of part ids"
    );
    doc
}

/// Probe 1: `xmi:XMI` names the Claim `c1` by `xmi:idref`.
fn probe_one() -> Document {
    writable(
        "probe 1",
        &format!(
            "<xmi:XMI{NS} xmi:idref=\"c1\"><sacm:ArgumentPackage xmi:id=\"p\"><name content=\"P\"/>\
             <argumentElement xmi:type=\"sacm:Claim\" xmi:id=\"c1\"><name content=\"C1\"/></argumentElement>\
             </sacm:ArgumentPackage></xmi:XMI>"
        ),
    )
}

/// Probe 2: the Claim `c2` cites `d1v`, the value id of the content of the
/// Description `d1` of the Claim `c1`.
fn probe_two() -> Document {
    writable(
        "probe 2",
        &format!(
            "<sacm:ArgumentPackage{NS} xmi:id=\"p\"><name content=\"P\"/>\
             <argumentElement xmi:type=\"sacm:Claim\" xmi:id=\"c1\"><name content=\"C1\"/>\
             <description xmi:id=\"d1\"><content><value xmi:id=\"d1v\" content=\"x\"/></content></description>\
             </argumentElement>\
             <argumentElement xmi:type=\"sacm:Claim\" xmi:id=\"c2\" isCitation=\"true\" citedElement=\"d1v\">\
             <name content=\"C2\"/></argumentElement></sacm:ArgumentPackage>"
        ),
    )
}

/// A Claim `id` with the attributes `attributes`, the name `id` and the
/// children `inner`.
fn claim(id: &str, attributes: &str, inner: &str) -> String {
    format!(
        "<argumentElement xmi:type=\"sacm:Claim\" xmi:id=\"{id}\"{attributes}><name content=\"{id}\"/>{inner}</argumentElement>"
    )
}

/// A document: `xmi:XMI` with the attributes `attributes` and the document
/// metadata `metadata`, then the root `ArgumentPackage` `p` holding
/// `elements`.
fn in_xmi(attributes: &str, metadata: &str, elements: &str) -> String {
    format!(
        "<xmi:XMI{NS}{attributes}>{metadata}<sacm:ArgumentPackage xmi:id=\"p\"><name content=\"P\"/>\
         {elements}</sacm:ArgumentPackage></xmi:XMI>"
    )
}

/// Every one of the 12 combinations of the delete policies, deleting
/// `element`.
fn every_policy(element: &str) -> Vec<DeleteElement> {
    let mut deletes = Vec::new();
    for reference in [
        ReferenceDeletePolicy::RejectIfReferenced,
        ReferenceDeletePolicy::DeleteReferencingRelationships,
        ReferenceDeletePolicy::ScrubReferences,
    ] {
        for package in [
            PackageDeletePolicy::RejectIfNonEmpty,
            PackageDeletePolicy::DeleteRecursively,
        ] {
            for cross in [
                CrossPackageReferencePolicy::RejectIfExternalReferencesExist,
                CrossPackageReferencePolicy::ApplyReferencePolicy,
            ] {
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

/// A delete of `element` that scrubs every reference and lets references
/// cross packages.
fn scrubbing(element: &str) -> DeleteElement {
    DeleteElement::new(element)
        .with_reference_policy(ReferenceDeletePolicy::ScrubReferences)
        .with_cross_package_policy(CrossPackageReferencePolicy::ApplyReferencePolicy)
}

/// Applies `operation`, which the test expects to be accepted, after
/// checking that `check` accepts it, that `preview` gives `apply`'s mutation,
/// and that `apply_preview` of that preview gives it too.
fn accepted(doc: &mut Document, operation: impl Into<Operation>) -> Mutation {
    let operation = operation.into();
    let checked = edit::check(doc, &operation);
    let preview = edit::preview(doc, &operation);
    let via_preview = preview
        .as_ref()
        .ok()
        .map(|preview| edit::apply_preview(&mut doc.clone(), preview));
    let mutation = match edit::apply(doc, operation.clone()) {
        Ok(mutation) => mutation,
        Err(error) => panic!("{ROW}: {operation:?} was refused: {error} ({error:?})"),
    };
    assert_eq!(
        (
            checked,
            preview.map(|preview| preview.mutation().clone()),
            via_preview
        ),
        (Ok(()), Ok(mutation.clone()), Some(Ok(mutation.clone()))),
        "{ROW}: check, preview or apply_preview answer {operation:?} otherwise than apply"
    );
    mutation
}

/// Applies `operation`, which the test expects to be refused, and returns
/// the refusal, after checking that it changed nothing (`==`, the revision
/// and the bytes both modes write) and that `check`, `preview` and
/// `apply_unchecked_text` refuse it alike.
fn refusal(doc: &mut Document, operation: &Operation) -> EditError {
    let before = doc.clone();
    let revision = doc.revision();
    let bytes = written(doc);
    let checked = edit::check(doc, operation);
    let previewed = edit::preview(doc, operation).map(|preview| preview.mutation().clone());
    let seam = edit::apply_unchecked_text(&mut doc.clone(), operation.clone());
    let error = match edit::apply(doc, operation.clone()) {
        Ok(mutation) => panic!("{ROW}: {operation:?} was accepted: {mutation:?}"),
        Err(error) => error,
    };
    assert!(
        *doc == before && doc.revision() == revision && written(doc) == bytes,
        "{ROW}: the refused {operation:?} changed the document, its revision or what it writes"
    );
    assert_eq!(
        (checked, previewed),
        (Err(error.clone()), Err(error.clone())),
        "{ROW}: check or preview answer {operation:?} otherwise than apply"
    );
    // The seam skips only LIB-EDIT-025's text check, which comes last.
    if !matches!(error, EditError::UnrepresentableCharacter { .. }) {
        assert_eq!(
            seam,
            Err(error.clone()),
            "{ROW}: apply_unchecked_text answers {operation:?} otherwise than apply"
        );
    }
    error
}

/// Applies `operation`, which the test expects LIB-EDIT-026 to refuse, and
/// returns what the refusal names, after [`refusal`]'s checks. The code is
/// SACM-REF-001 and the message one line in lower case.
fn refused(doc: &mut Document, operation: impl Into<Operation>) -> Named {
    let operation = operation.into();
    let error = refusal(doc, &operation);
    assert_eq!(
        error.code(),
        Code::REF_DANGLING,
        "{ROW}: the code of the refusal of {operation:?}: {error:?}"
    );
    let shown = error.to_string();
    assert!(
        !shown.contains('\n') && shown.starts_with(char::is_lowercase),
        "{ROW}: the message {shown:?} is not one line starting in lower case"
    );
    named(&error).unwrap_or_else(|| {
        panic!(
            "{ROW}: {operation:?} is not refused as PartReference or MetadataReference: {error:?}"
        )
    })
}

/// The variant name of `error`, from its `Debug`.
fn variant(error: &EditError) -> String {
    format!("{error:?}")
        .split([' ', '{', '('])
        .next()
        .unwrap_or_default()
        .to_owned()
}

/// What a `Referenced` or `CrossPackageReference` refusal names: the
/// variant, the holder, the end and the id.
fn policy_refusal(error: &EditError) -> Option<(&'static str, String, &'static str, String)> {
    match error {
        EditError::Referenced {
            holder, end, id, ..
        } => Some((
            "Referenced",
            holder.as_str().to_owned(),
            *end,
            id.as_str().to_owned(),
        )),
        EditError::CrossPackageReference {
            holder, end, id, ..
        } => Some((
            "CrossPackageReference",
            holder.as_str().to_owned(),
            *end,
            id.as_str().to_owned(),
        )),
        _ => None,
    }
}

// ---------------------------------------------------------------------------
// 1-3: the probes and the fixture
// ---------------------------------------------------------------------------

#[test]
fn lib_edit_026_deleting_an_element_xmi_metadata_names_is_refused_under_every_policy() {
    let start = probe_one();
    for delete in every_policy("c1") {
        let mut doc = start.clone();
        assert_eq!(
            refused(&mut doc, delete.clone()),
            meta(None, "xmi:XMI", "xmi:idref", "c1"),
            "{ROW}: probe 1, {delete:?}"
        );
        still_strict(&format!("probe 1 after the refused {delete:?}"), &doc);
    }
    // The fixture: an xmi:Extension of c2 names t1's content value t1v, an
    // xmi:documentation names c3 and an xmi:Difference names c4.
    let start = fixture();
    for (element, expected) in [
        ("t1", meta(Some("c2"), "xmi:Extension", "xmi:idref", "t1v")),
        ("c3", meta(None, "xmi:documentation", "xmi:idref", "c3")),
        ("c4", meta(None, "xmi:Difference", "target", "c4")),
    ] {
        for delete in every_policy(element) {
            let mut doc = start.clone();
            assert_eq!(
                refused(&mut doc, delete.clone()),
                expected,
                "{ROW}: the fixture, {delete:?}"
            );
            still_strict(&format!("the fixture after the refused {delete:?}"), &doc);
        }
    }
}

#[test]
fn lib_edit_026_emptying_a_content_whose_value_a_reference_names_is_refused() {
    let start = probe_two();
    for operation in [
        SetContent::new("d1", MultiLangString::default()),
        SetContent::remove("d1"),
    ] {
        let mut doc = start.clone();
        assert_eq!(
            refused(&mut doc, operation.clone()),
            part("c2", "citedElement", "d1v"),
            "{ROW}: probe 2, {operation:?}"
        );
        still_strict(&format!("probe 2 after the refused {operation:?}"), &doc);
    }
    // A set keeping one value keeps its id by position.
    let mut doc = start.clone();
    accepted(&mut doc, SetContent::new("d1", en("Kept")));
    assert_eq!(
        doc.get("d1").map(|d1| d1.content_value_ids().to_vec()),
        Some(vec![Some(ElementId::from("d1v"))]),
        "{ROW}: the value kept its id"
    );
    still_strict("probe 2 after a set keeping one value", &doc);

    // The fixture: one value frees d1v2, which the metadata of d1's content
    // names; two keep both ids.
    let mut doc = fixture();
    assert_eq!(
        refused(&mut doc, SetContent::new("d1", en("Only"))),
        meta(Some("d1"), "xmi:Extension", "xmi:idref", "d1v2"),
        "{ROW}: the fixture, one value for d1"
    );
    accepted(
        &mut doc,
        SetContent::new("d1", multi(&[en("One"), sv("Två")])),
    );
    still_strict("the fixture after a set keeping two values", &doc);
}

#[test]
fn lib_edit_026_a_delete_that_removes_a_part_id_a_reference_names_is_refused_under_every_policy() {
    let start = probe_two();
    for element in ["d1", "c1"] {
        for delete in every_policy(element) {
            let mut doc = start.clone();
            assert_eq!(
                refused(&mut doc, delete.clone()),
                part("c2", "citedElement", "d1v"),
                "{ROW}: probe 2, {delete:?}"
            );
            still_strict(&format!("probe 2 after the refused {delete:?}"), &doc);
        }
    }
    // The fixture: c2 cites d1's first value, r2 cites r1's location value
    // (from another package).
    let start = fixture();
    for (element, expected) in [
        ("c1", part("c2", "citedElement", "d1v1")),
        ("d1", part("c2", "citedElement", "d1v1")),
        ("r1", part("r2", "citedElement", "r1lv")),
    ] {
        for delete in every_policy(element) {
            let mut doc = start.clone();
            assert_eq!(
                refused(&mut doc, delete.clone()),
                expected,
                "{ROW}: the fixture, {delete:?}"
            );
            still_strict(&format!("the fixture after the refused {delete:?}"), &doc);
        }
    }
}

// ---------------------------------------------------------------------------
// 3: every part id, every text operation and every reference end
// ---------------------------------------------------------------------------

/// The document of [`lib_edit_026_every_part_id_and_every_text_operation_is_checked`]:
/// the Claim `c1` with the name `n1`, the Description `d1` (content `d1c`,
/// value `d1v`) and the `TaggedValue` `t1` (key `t1k`, key value `t1kv`,
/// content `t1c`, content value `t1cv`), the Resource `r1` (location `r1l`,
/// value `r1lv`) in another package, each part with an `xmi:Extension` whose
/// id is the part's with `x`; and the Claim `cx` citing `cited`.
fn parts(cited: &str) -> String {
    let x = |id: &str| format!("<xmi:Extension xmi:id=\"{id}x\"/>");
    format!(
        "<xmi:XMI{NS}><sacm:ArgumentPackage xmi:id=\"p\"><name content=\"P\"/>\
         <argumentElement xmi:type=\"sacm:Claim\" xmi:id=\"c1\"><name xmi:id=\"n1\" content=\"C1\">{}</name>\
         <description xmi:id=\"d1\"><content xmi:id=\"d1c\">{}<value xmi:id=\"d1v\" content=\"a\">{}</value></content></description>\
         <taggedValue xmi:id=\"t1\"><key xmi:id=\"t1k\">{}<value xmi:id=\"t1kv\" content=\"K\">{}</value></key>\
         <content xmi:id=\"t1c\">{}<value xmi:id=\"t1cv\" content=\"V\">{}</value></content></taggedValue>\
         </argumentElement>\
         <argumentElement xmi:type=\"sacm:Claim\" xmi:id=\"cx\" isCitation=\"true\" citedElement=\"{cited}\"><name content=\"X\"/></argumentElement>\
         </sacm:ArgumentPackage>\
         <sacm:ArtifactPackage xmi:id=\"e\"><name content=\"E\"/>\
         <artifactElement xmi:type=\"sacm:Resource\" xmi:id=\"r1\"><name content=\"R1\"/>\
         <location xmi:id=\"r1l\">{}<value xmi:id=\"r1lv\" content=\"u\">{}</value></location></artifactElement>\
         </sacm:ArtifactPackage></xmi:XMI>",
        x("n1"),
        x("d1c"),
        x("d1v"),
        x("t1k"),
        x("t1kv"),
        x("t1c"),
        x("t1cv"),
        x("r1l"),
        x("r1lv"),
    )
}

/// The operations of the part-id matrix, by label.
fn part_operations() -> Vec<(&'static str, Operation)> {
    vec![
        ("delete c1", DeleteElement::new("c1").into()),
        ("delete c1, scrubbing", scrubbing("c1").into()),
        ("delete d1", DeleteElement::new("d1").into()),
        ("delete d1, scrubbing", scrubbing("d1").into()),
        ("delete t1", DeleteElement::new("t1").into()),
        ("delete t1, scrubbing", scrubbing("t1").into()),
        ("delete r1", DeleteElement::new("r1").into()),
        ("delete r1, scrubbing", scrubbing("r1").into()),
        (
            "SetContent d1, no value",
            SetContent::new("d1", MultiLangString::default()).into(),
        ),
        (
            "SetContent d1, one value",
            SetContent::new("d1", en("b")).into(),
        ),
        ("SetContent::remove d1", SetContent::remove("d1").into()),
        (
            "SetKey t1, no value",
            SetKey::new("t1", MultiLangString::default()).into(),
        ),
        ("SetKey t1, one value", SetKey::new("t1", en("K2")).into()),
        (
            "SetContent t1, no value",
            SetContent::new("t1", MultiLangString::default()).into(),
        ),
        ("SetContent::remove t1", SetContent::remove("t1").into()),
        (
            "SetTaggedValue c1 K, no value",
            SetTaggedValue::new("c1", "K", MultiLangString::default()).into(),
        ),
        (
            "SetTaggedValue c1 K, one value",
            SetTaggedValue::new("c1", "K", en("V2")).into(),
        ),
        (
            "SetLocation r1, no value",
            SetLocation::new("r1", MultiLangString::default()).into(),
        ),
        (
            "SetLocation r1, one value",
            SetLocation::new("r1", en("w")).into(),
        ),
        ("SetLocation::remove r1", SetLocation::remove("r1").into()),
    ]
}

/// An id of [`parts`] that `cx` cites, the labels of the operations that
/// remove it, and the labels of the deletes a delete policy refuses, with
/// the variant.
type PartCase = (
    &'static str,
    Vec<&'static str>,
    Vec<(&'static str, &'static str)>,
);

/// For each id of [`parts`]: the operations that remove it, so that the
/// citation refuses them as `PartReference`; and the deletes that a delete
/// policy refuses, as before, for a metadata id, which is the element's own
/// (ADR 0042 D5), with the variant. Every other operation is accepted.
fn part_matrix() -> Vec<PartCase> {
    const C1: [&str; 2] = ["delete c1", "delete c1, scrubbing"];
    const D1: [&str; 2] = ["delete d1", "delete d1, scrubbing"];
    const T1: [&str; 2] = ["delete t1", "delete t1, scrubbing"];
    const R1: [&str; 2] = ["delete r1", "delete r1, scrubbing"];
    let with = |deletes: &[&[&'static str]], sets: &[&'static str]| -> Vec<&'static str> {
        deletes
            .iter()
            .flat_map(|deletes| deletes.iter().copied())
            .chain(sets.iter().copied())
            .collect()
    };
    let referenced = |deletes: &[&'static str]| -> Vec<(&'static str, &'static str)> {
        deletes
            .iter()
            .map(|delete| (*delete, "Referenced"))
            .collect()
    };
    vec![
        // The name: only a delete removes it.
        ("n1", with(&[&C1], &[]), vec![]),
        ("n1x", vec![], referenced(&["delete c1"])),
        // A content keeps its id when it is set, even to no value.
        ("d1c", with(&[&C1, &D1], &["SetContent::remove d1"]), vec![]),
        (
            "d1cx",
            vec!["SetContent::remove d1"],
            referenced(&["delete c1", "delete d1"]),
        ),
        (
            "d1v",
            with(
                &[&C1, &D1],
                &["SetContent d1, no value", "SetContent::remove d1"],
            ),
            vec![],
        ),
        (
            "d1vx",
            vec!["SetContent d1, no value", "SetContent::remove d1"],
            referenced(&["delete c1", "delete d1"]),
        ),
        // A key is required and keeps its id.
        ("t1k", with(&[&C1, &T1], &[]), vec![]),
        ("t1kx", vec![], referenced(&["delete c1", "delete t1"])),
        ("t1kv", with(&[&C1, &T1], &["SetKey t1, no value"]), vec![]),
        (
            "t1kvx",
            vec!["SetKey t1, no value"],
            referenced(&["delete c1", "delete t1"]),
        ),
        ("t1c", with(&[&C1, &T1], &["SetContent::remove t1"]), vec![]),
        (
            "t1cx",
            vec!["SetContent::remove t1"],
            referenced(&["delete c1", "delete t1"]),
        ),
        (
            "t1cv",
            with(
                &[&C1, &T1],
                &[
                    "SetContent t1, no value",
                    "SetContent::remove t1",
                    "SetTaggedValue c1 K, no value",
                ],
            ),
            vec![],
        ),
        (
            "t1cvx",
            vec![
                "SetContent t1, no value",
                "SetContent::remove t1",
                "SetTaggedValue c1 K, no value",
            ],
            referenced(&["delete c1", "delete t1"]),
        ),
        // r1 is in another package than cx.
        ("r1l", with(&[&R1], &["SetLocation::remove r1"]), vec![]),
        (
            "r1lx",
            vec!["SetLocation::remove r1"],
            vec![("delete r1", "CrossPackageReference")],
        ),
        (
            "r1lv",
            with(
                &[&R1],
                &["SetLocation r1, no value", "SetLocation::remove r1"],
            ),
            vec![],
        ),
        (
            "r1lvx",
            vec!["SetLocation r1, no value", "SetLocation::remove r1"],
            vec![("delete r1", "CrossPackageReference")],
        ),
    ]
}

/// The XMI of a holder of `end` naming `v`, and the element of
/// [`ends`] it goes in: `p`, `a`, `t` or `case`.
fn holder_of(end: ReferenceEnd) -> (&'static str, String) {
    let element = |role: &str, class: &str, attributes: &str| {
        format!(
            "<{role} xmi:type=\"sacm:{class}\" xmi:id=\"h\"{attributes}><name content=\"H\"/></{role}>"
        )
    };
    let argument = |class: &str, attributes: &str| element("argumentElement", class, attributes);
    let artifact = |class: &str, attributes: &str| element("artifactElement", class, attributes);
    let term = |class: &str, attributes: &str| element("terminologyElement", class, attributes);
    let package = |class: &str, attributes: &str| element("argumentPackage", class, attributes);
    match end {
        ReferenceEnd::CitedElement => ("p", argument("Claim", " isCitation=\"true\" citedElement=\"v\"")),
        ReferenceEnd::AbstractForm => ("p", argument("Claim", " isAbstract=\"true\" abstractForm=\"v\"")),
        ReferenceEnd::MetaClaim => ("p", argument("Claim", " metaClaim=\"v\"")),
        ReferenceEnd::Reasoning => (
            "p",
            argument("AssertedInference", " source=\"c0\" target=\"c0\" reasoning=\"v\""),
        ),
        ReferenceEnd::Source => ("p", argument("AssertedInference", " source=\"v\" target=\"c0\"")),
        ReferenceEnd::Target => ("p", argument("AssertedInference", " source=\"c0\" target=\"v\"")),
        ReferenceEnd::Structure => ("p", argument("ArgumentReasoning", " structure=\"v\"")),
        ReferenceEnd::ReferencedArtifactElement => (
            "p",
            argument("ArtifactReference", " referencedArtifactElement=\"v\""),
        ),
        ReferenceEnd::GroupMember => ("p", argument("ArgumentGroup", " argumentElement=\"v\"")),
        ReferenceEnd::ArtifactGroupMember => ("a", artifact("ArtifactGroup", " artifactElement=\"v\"")),
        ReferenceEnd::ArtifactSource => (
            "a",
            artifact("ArtifactAssetRelationship", " source=\"v\" target=\"r0\""),
        ),
        ReferenceEnd::ArtifactTarget => (
            "a",
            artifact("ArtifactAssetRelationship", " source=\"r0\" target=\"v\""),
        ),
        ReferenceEnd::Category => ("t", term("Term", " value=\"h\" category=\"v\"")),
        ReferenceEnd::Element => ("t", term("Expression", " value=\"h\" element=\"v\"")),
        ReferenceEnd::Origin => ("t", term("Term", " value=\"h\" origin=\"v\"")),
        ReferenceEnd::TerminologyGroupMember => {
            ("t", term("TerminologyGroup", " terminologyElement=\"v\""))
        }
        ReferenceEnd::Interface => ("case", package("ArgumentPackage", " interface=\"v\"")),
        ReferenceEnd::Implements => ("case", package("ArgumentPackageInterface", " implements=\"v\"")),
        ReferenceEnd::ParticipantPackage => (
            "case",
            package("ArgumentPackageBinding", " participantPackage=\"p v\""),
        ),
        ReferenceEnd::Expression => (
            "p",
            "<argumentElement xmi:type=\"sacm:Claim\" xmi:id=\"h\"><name xmi:type=\"sacm:ExpressionLangString\" \
             content=\"H\" expression=\"v\"/></argumentElement>"
                .to_owned(),
        ),
        other => panic!("{ROW}: setup: no holder for the end {other:?}"),
    }
}

/// The AssuranceCasePackage `case` holding the ArgumentPackage `p` (the
/// Claim `c0` with the Note `n`, whose content value is `v`), the
/// ArtifactPackage `a` (the Artifact `r0`) and the TerminologyPackage `t`,
/// with the holder of `end` naming `v`.
fn ends(end: ReferenceEnd) -> String {
    let (place, holder) = holder_of(end);
    let at = |here: &str| if place == here { holder.as_str() } else { "" };
    format!(
        "<sacm:AssuranceCasePackage{NS} xmi:id=\"case\"><name content=\"Case\"/>\
         <argumentPackage xmi:id=\"p\"><name content=\"P\"/>\
         <argumentElement xmi:type=\"sacm:Claim\" xmi:id=\"c0\"><name content=\"C0\"/>\
         <note xmi:id=\"n\"><content><value xmi:id=\"v\" content=\"x\"/></content></note></argumentElement>{}\
         </argumentPackage>\
         <artifactPackage xmi:id=\"a\"><name content=\"A\"/>\
         <artifactElement xmi:type=\"sacm:Artifact\" xmi:id=\"r0\"><name content=\"R0\"/></artifactElement>{}\
         </artifactPackage>\
         <terminologyPackage xmi:id=\"t\"><name content=\"T\"/>{}</terminologyPackage>{}\
         </sacm:AssuranceCasePackage>",
        at("p"),
        at("a"),
        at("t"),
        at("case")
    )
}

#[test]
fn lib_edit_026_every_part_id_and_every_text_operation_is_checked() {
    // Every id of a name, content, key, location and value, and of the
    // metadata in each, cited by cx; every operation that can remove one.
    let operations = part_operations();
    for (cited, removing, policies) in part_matrix() {
        let start = writable(&format!("the parts, cx citing {cited}"), &parts(cited));
        for (label, operation) in &operations {
            let what = format!("cx citing {cited}, {label}");
            let mut doc = start.clone();
            if removing.contains(label) {
                assert_eq!(
                    refused(&mut doc, operation.clone()),
                    part("cx", "citedElement", cited),
                    "{ROW}: {what}"
                );
            } else if let Some((_, expected)) = policies.iter().find(|(delete, _)| delete == label)
            {
                // A reference to a deleted element's metadata id stays the
                // delete policies'.
                let error = refusal(&mut doc, operation);
                assert_eq!(
                    policy_refusal(&error),
                    Some((*expected, "cx".to_owned(), "citedElement", cited.to_owned())),
                    "{ROW}: {what}: not the policy's refusal: {error:?}"
                );
            } else {
                accepted(&mut doc, operation.clone());
            }
            still_strict(&what, &doc);
        }
    }
    // Every reference end, the derived `expression` included, naming the
    // value id v of the Note n, which SetContent::remove removes.
    for &end in ReferenceEnd::ALL {
        let what = format!("the end {}", end.name());
        let start = writable(&what, &ends(end));
        let referrers: Vec<(String, ReferenceEnd)> = start
            .referrers("v")
            .iter()
            .map(|referrer| (referrer.holder().id().as_str().to_owned(), referrer.end()))
            .collect();
        assert_eq!(
            referrers,
            [("h".to_owned(), end)],
            "{ROW}: setup: {what}: the referrers of v"
        );
        let mut doc = start.clone();
        assert_eq!(
            refused(&mut doc, SetContent::remove("n")),
            part("h", end.name(), "v"),
            "{ROW}: {what}"
        );
        // Keeping the value keeps its id: accepted.
        accepted(&mut doc, SetContent::new("n", en("y")));
        still_strict(&what, &doc);
    }
}

// ---------------------------------------------------------------------------
// 4: the metadata tokens that count
// ---------------------------------------------------------------------------

/// The Claims `c1` and `c2` in `p`, with the metadata `metadata` of the
/// document and the attributes `attributes` of `xmi:XMI`.
fn named_by(attributes: &str, metadata: &str) -> String {
    in_xmi(
        attributes,
        metadata,
        &(claim("c1", "", "") + &claim("c2", "", "")),
    )
}

#[test]
#[expect(
    clippy::too_many_lines,
    reason = "the tokens that count and those that do not read best as one list each"
)]
fn lib_edit_026_every_metadata_idref_a_strict_write_checks_counts_and_no_other() {
    // The xmi:idref of xmi:XMI.
    let mut doc = writable("xmi:XMI xmi:idref", &named_by(" xmi:idref=\"c1\"", ""));
    assert_eq!(
        refused(&mut doc, DeleteElement::new("c1")),
        meta(None, "xmi:XMI", "xmi:idref", "c1"),
        "{ROW}: the xmi:idref of xmi:XMI"
    );
    // Each IDREF and IDREFS attribute XMI.xsd gives an element, by its type,
    // and the children it walks: (metadata, element, attribute).
    let counted = [
        (
            "<xmi:documentation xmi:idref=\"c1\"/>",
            "xmi:documentation",
            "xmi:idref",
        ),
        (
            "<xmi:Documentation xmi:idref=\"c1\"/>",
            "xmi:Documentation",
            "xmi:idref",
        ),
        (
            "<xmi:extension xmi:idref=\"c1\"/>",
            "xmi:extension",
            "xmi:idref",
        ),
        (
            "<xmi:Extension xmi:idref=\"c1\"/>",
            "xmi:Extension",
            "xmi:idref",
        ),
        (
            "<xmi:difference target=\"c1\"/>",
            "xmi:difference",
            "target",
        ),
        (
            "<xmi:Difference target=\"c1\"/>",
            "xmi:Difference",
            "target",
        ),
        (
            "<xmi:Difference container=\"c1\"/>",
            "xmi:Difference",
            "container",
        ),
        (
            "<xmi:Difference xmi:idref=\"c1\"/>",
            "xmi:Difference",
            "xmi:idref",
        ),
        ("<xmi:Delete target=\"c1\"/>", "xmi:Delete", "target"),
        ("<xmi:Delete container=\"c1\"/>", "xmi:Delete", "container"),
        ("<xmi:Add addition=\"c1\"/>", "xmi:Add", "addition"),
        ("<xmi:Add target=\"c1\"/>", "xmi:Add", "target"),
        ("<xmi:Add container=\"c1\"/>", "xmi:Add", "container"),
        (
            "<xmi:Replace replacement=\"c1\"/>",
            "xmi:Replace",
            "replacement",
        ),
        ("<xmi:Replace target=\"c1\"/>", "xmi:Replace", "target"),
        // A token of an IDREFS list, after one that stays.
        (
            "<xmi:Difference target=\"p  c1\"/>",
            "xmi:Difference",
            "target",
        ),
        // The children walked.
        (
            "<xmi:Difference><difference target=\"c1\"/></xmi:Difference>",
            "difference",
            "target",
        ),
        (
            "<xmi:Difference><container container=\"c1\"/></xmi:Difference>",
            "container",
            "container",
        ),
        (
            "<xmi:Difference><xmi:Extension xmi:idref=\"c1\"/></xmi:Difference>",
            "xmi:Extension",
            "xmi:idref",
        ),
        (
            "<xmi:Documentation><xmi:Extension xmi:idref=\"c1\"/></xmi:Documentation>",
            "xmi:Extension",
            "xmi:idref",
        ),
        (
            "<xmi:Extension><xmi:Add addition=\"c1\"/></xmi:Extension>",
            "xmi:Add",
            "addition",
        ),
    ];
    for (metadata, element, attribute) in counted {
        let mut doc = writable(metadata, &named_by("", metadata));
        assert_eq!(
            refused(&mut doc, DeleteElement::new("c1")),
            meta(None, element, attribute, "c1"),
            "{ROW}: {metadata}"
        );
    }
    // Attributes XMI.xsd does not type IDREF or IDREFS on that element,
    // elements it does not walk, anything in a Difference's target
    // (SACM23-XMI-031), and an href: a delete of c1 is accepted and the
    // document still writes strictly.
    let not_counted = [
        "<xmi:Difference addition=\"c1\" replacement=\"c1\"/>",
        "<xmi:Add replacement=\"c1\"/>",
        "<xmi:Replace addition=\"c1\"/>",
        "<xmi:Documentation target=\"c1\" container=\"c1\"/>",
        "<xmi:Extension target=\"c1\"/>",
        "<xmi:Extension><v target=\"c1\" xmi:idref=\"c1\"/></xmi:Extension>",
        "<xmi:Documentation><v xmi:idref=\"c1\"/></xmi:Documentation>",
        "<xmi:Difference><target xmi:idref=\"c1\" target=\"c1\"><x container=\"c1\"/></target></xmi:Difference>",
        "<xmi:Extension><xmi:Difference><target><xmi:Extension xmi:idref=\"c1\"/></target></xmi:Difference></xmi:Extension>",
        "<xmi:Extension href=\"#c1\"/>",
    ];
    for metadata in not_counted {
        let mut doc = writable(metadata, &named_by("", metadata));
        accepted(&mut doc, DeleteElement::new("c1"));
        still_strict(&format!("{metadata}, c1 deleted"), &doc);
    }
    // Preserved content, which a tolerant read keeps: not checked.
    let input = format!(
        "<xmi:XMI{NS} xmlns:v=\"urn:example:v\"><sacm:ArgumentPackage xmi:id=\"p\"><name content=\"P\"/>{}{}\
         </sacm:ArgumentPackage></xmi:XMI>",
        claim("c1", "", ""),
        claim(
            "c2",
            " v:ref=\"c1\"",
            "<v:x xmi:idref=\"c1\" target=\"c1\"/>"
        )
    );
    let mut doc = read_in("preserved content naming c1", Mode::Tolerant, &input);
    assert!(
        doc.has_preserved_content(),
        "{ROW}: setup: no preserved content"
    );
    accepted(&mut doc, DeleteElement::new("c1"));
}

#[test]
fn lib_edit_026_metadata_of_the_document_an_element_a_name_a_text_and_a_value_counts() {
    let x = "<xmi:Extension xmi:idref=\"c1\"/>";
    // (where, the document, the holder)
    let place = |document: &str,
                 own: &str,
                 name: &str,
                 text: &str,
                 value: &str,
                 key: &str,
                 location: &str| {
        format!(
            "<xmi:XMI{NS}>{document}<sacm:ArgumentPackage xmi:id=\"p\"><name content=\"P\"/>{}\
             <argumentElement xmi:type=\"sacm:Claim\" xmi:id=\"c2\"><name content=\"C2\">{name}</name>{own}\
             <description xmi:id=\"d2\"><content>{text}<value content=\"x\">{value}</value></content></description>\
             <taggedValue xmi:id=\"t2\"><key><value content=\"k\">{key}</value></key></taggedValue>\
             </argumentElement></sacm:ArgumentPackage>\
             <sacm:ArtifactPackage xmi:id=\"e\"><name content=\"E\"/>\
             <artifactElement xmi:type=\"sacm:Resource\" xmi:id=\"r1\"><name content=\"R1\"/>\
             <location>{location}<value content=\"u\"/></location></artifactElement></sacm:ArtifactPackage></xmi:XMI>",
            claim("c1", "", "")
        )
    };
    let cases = [
        ("the document", place(x, "", "", "", "", "", ""), None),
        ("an element", place("", x, "", "", "", "", ""), Some("c2")),
        ("a name", place("", "", x, "", "", "", ""), Some("c2")),
        ("a content", place("", "", "", x, "", "", ""), Some("d2")),
        (
            "a content value",
            place("", "", "", "", x, "", ""),
            Some("d2"),
        ),
        ("a key value", place("", "", "", "", "", x, ""), Some("t2")),
        ("a location", place("", "", "", "", "", "", x), Some("r1")),
    ];
    for (what, input, holder) in cases {
        let start = writable(&format!("metadata of {what}"), &input);
        for delete in every_policy("c1") {
            let mut doc = start.clone();
            assert_eq!(
                refused(&mut doc, delete.clone()),
                meta(holder, "xmi:Extension", "xmi:idref", "c1"),
                "{ROW}: metadata of {what}, {delete:?}"
            );
        }
    }
    // The document's metadata names x1, the xmi:id of an xmi:Extension in
    // c1, which goes with c1.
    let input = in_xmi(
        "",
        "<xmi:Extension xmi:idref=\"x1\"/>",
        &(claim("c1", "", "<xmi:Extension xmi:id=\"x1\"/>") + &claim("c2", "", "")),
    );
    let start = writable("metadata naming an id of c1's metadata", &input);
    for delete in every_policy("c1") {
        assert_eq!(
            refused(&mut start.clone(), delete.clone()),
            meta(None, "xmi:Extension", "xmi:idref", "x1"),
            "{ROW}: metadata naming an id of c1's metadata, {delete:?}"
        );
    }
    metadata_naming_preserved_and_edited_ids();
}

/// A delete frees the ids of preserved content too, and a text set leaves
/// the edited element's own metadata, its name's and its other texts'.
fn metadata_naming_preserved_and_edited_ids() {
    // c1 holds preserved content with the id px, which the document's
    // metadata names.
    let input = format!(
        "<xmi:XMI{NS} xmlns:v=\"urn:example:v\"><xmi:Extension xmi:idref=\"px\"/>\
         <sacm:ArgumentPackage xmi:id=\"p\"><name content=\"P\"/>{}{}</sacm:ArgumentPackage></xmi:XMI>",
        claim("c1", "", "<v:x xmi:id=\"px\"/>"),
        claim("c2", "", "")
    );
    let start = read_in("metadata naming preserved content", Mode::Tolerant, &input);
    assert!(
        start.has_preserved_content(),
        "{ROW}: setup: no preserved content"
    );
    for delete in every_policy("c1")
        .into_iter()
        .map(|delete| delete.with_package_policy(PackageDeletePolicy::DeleteRecursively))
    {
        assert_eq!(
            refused(&mut start.clone(), delete.clone()),
            meta(None, "xmi:Extension", "xmi:idref", "px"),
            "{ROW}: metadata naming preserved content, {delete:?}"
        );
    }
    // The edited element's own metadata, its name's and its other texts'
    // stay, and name the freed value.
    let input = in_xmi(
        "",
        "",
        &claim(
            "c1",
            "",
            "<description xmi:id=\"d1\"><xmi:Extension xmi:idref=\"v2\"/><content>\
             <value xmi:id=\"v1\" content=\"a\"/><value xmi:id=\"v2\" content=\"b\"/></content>\
             </description>\
             <taggedValue xmi:id=\"t1\"><key><value xmi:id=\"k1\" content=\"K\"/>\
             <value xmi:id=\"k2\" content=\"L\"/></key>\
             <content><xmi:Extension xmi:idref=\"k2\"/><value content=\"V\"/></content></taggedValue>",
        ),
    )
    .replace(
        "</xmi:XMI>",
        "<sacm:ArtifactPackage xmi:id=\"e\"><name content=\"E\"/>\
         <artifactElement xmi:type=\"sacm:Resource\" xmi:id=\"r1\">\
         <name content=\"R1\"><xmi:Extension xmi:idref=\"lw\"/></name>\
         <location><value xmi:id=\"lv\" content=\"u\"/><value xmi:id=\"lw\" content=\"w\"/></location>\
         </artifactElement></sacm:ArtifactPackage></xmi:XMI>",
    );
    let start = writable("metadata of the edited element", &input);
    for (operation, expected) in [
        (
            Operation::from(SetContent::new("d1", LangString::new("x"))),
            meta(Some("d1"), "xmi:Extension", "xmi:idref", "v2"),
        ),
        (
            SetLocation::new("r1", LangString::new("x")).into(),
            meta(Some("r1"), "xmi:Extension", "xmi:idref", "lw"),
        ),
        (
            SetKey::new("t1", LangString::new("K")).into(),
            meta(Some("t1"), "xmi:Extension", "xmi:idref", "k2"),
        ),
    ] {
        assert_eq!(
            refused(&mut start.clone(), operation.clone()),
            expected,
            "{ROW}: metadata of the edited element, {operation:?}"
        );
    }
}

// ---------------------------------------------------------------------------
// 5 and 6: what does not refuse
// ---------------------------------------------------------------------------

#[test]
fn lib_edit_026_what_goes_with_the_edit_or_named_nothing_before_does_not_refuse() {
    // A reference and metadata inside the deleted subtree: c1 cites its own
    // description's value and its metadata names it and c1.
    let input = in_xmi(
        "",
        "",
        &(claim(
            "c1",
            " isCitation=\"true\" citedElement=\"d1v\"",
            "<xmi:Extension><xmi:Difference target=\"d1v c1 d1\"/></xmi:Extension>\
             <description xmi:id=\"d1\"><content><xmi:Extension xmi:idref=\"d1v\"/>\
             <value xmi:id=\"d1v\" content=\"x\"/></content></description>",
        ) + &claim("c2", "", "")),
    );
    let start = writable("references inside the subtree", &input);
    for delete in every_policy("c1") {
        let mut doc = start.clone();
        accepted(&mut doc, delete.clone());
        still_strict(&format!("references inside the subtree, {delete:?}"), &doc);
    }
    // d1 alone is still named by c1.
    for delete in every_policy("d1") {
        assert_eq!(
            refused(&mut start.clone(), delete.clone()),
            part("c1", "citedElement", "d1v"),
            "{ROW}: d1 deleted alone, {delete:?}"
        );
    }
    // Metadata in a removed value, naming that value and another removed one.
    let input = in_xmi(
        "",
        "",
        &claim(
            "c1",
            "",
            "<description xmi:id=\"d1\"><content>\
             <value xmi:id=\"v1\" content=\"a\"/>\
             <value xmi:id=\"v2\" content=\"b\"><xmi:Extension><xmi:Difference target=\"v2 v3\"/></xmi:Extension></value>\
             <value xmi:id=\"v3\" content=\"c\"><xmi:Extension xmi:idref=\"v2\"/></value>\
             </content></description>",
        ),
    );
    let start = writable("metadata in removed values", &input);
    for operation in [
        Operation::from(SetContent::new("d1", en("a"))),
        SetContent::new("d1", MultiLangString::default()).into(),
        SetContent::remove("d1").into(),
        DeleteElement::new("d1").into(),
    ] {
        let mut doc = start.clone();
        accepted(&mut doc, operation.clone());
        still_strict(&format!("metadata in removed values, {operation:?}"), &doc);
    }
    // A token and a reference that named nothing before: the document is
    // not strictly writable, and stays as it was.
    let input = in_xmi(
        " xmi:idref=\"nope\"",
        "<xmi:Difference target=\"gone c1\"/>",
        &(claim(
            "c1",
            "",
            "<description xmi:id=\"d1\"><content><value xmi:id=\"d1v\" content=\"x\"/></content></description>",
        ) + &claim("c2", " isCitation=\"true\" citedElement=\"missing\"", "")),
    );
    let start = read_in("names that named nothing", Mode::Strict, &input);
    let refused_before = written(&start)[1].clone();
    assert!(
        refused_before.is_err(),
        "{ROW}: setup: a strict write accepts names that named nothing"
    );
    let mut doc = start.clone();
    accepted(&mut doc, SetContent::remove("d1"));
    assert_eq!(
        written(&doc)[1],
        refused_before,
        "{ROW}: names that named nothing, SetContent::remove: the strict write refuses otherwise"
    );
    // But the token that names c1 still refuses c1's delete.
    assert_eq!(
        refused(&mut start.clone(), DeleteElement::new("c1")),
        meta(None, "xmi:Difference", "target", "c1"),
        "{ROW}: names that named nothing, c1's delete"
    );
    // A set that changes nothing removes nothing: an empty mutation.
    let start = fixture();
    let held = |doc: &Document, id: &str| -> MultiLangString {
        doc.get(id)
            .and_then(|element| element.content().or_else(|| element.location()))
            .cloned()
            .unwrap_or_else(|| panic!("{ROW}: setup: {id} holds no text"))
    };
    let d1 = held(&start, "d1");
    let t1 = held(&start, "t1");
    let r1 = held(&start, "r1");
    for operation in [
        Operation::from(SetContent::new("d1", d1)),
        SetContent::new("t1", t1.clone()).into(),
        SetTaggedValue::new("c1", "ASIL", t1).into(),
        SetLocation::new("r1", r1).into(),
    ] {
        let mut doc = start.clone();
        let mutation = accepted(&mut doc, operation.clone());
        assert!(
            mutation.is_empty(),
            "{ROW}: a set of the value held, {operation:?}, is not an empty mutation: {mutation:?}"
        );
        still_strict(&format!("the fixture, {operation:?}"), &doc);
    }
    expressions_that_go_with_the_text();
}

/// An `ExpressionLangString` value with the id `id`, the content `content`
/// and the expression `expression`, as written in a text.
fn expression_value_xmi(id: &str, content: &str, expression: &str) -> String {
    format!(
        "<value xmi:type=\"sacm:ExpressionLangString\" xmi:id=\"{id}\" content=\"{content}\" \
         expression=\"{expression}\"/>"
    )
}

/// The `expression` of a value the set removes or replaces goes with it: it
/// refuses nothing, while one the element still holds after the set does.
#[expect(
    clippy::too_many_lines,
    reason = "the accepted and the refused cases read best side by side"
)]
fn expressions_that_go_with_the_text() {
    let ev = expression_value_xmi;
    let input = in_xmi(
        "",
        "",
        &(claim(
            "c1",
            "",
            &format!(
                "<description xmi:id=\"d1\"><content>{}<value xmi:id=\"v2\" content=\"b\"/></content>\
                 </description>\
                 <taggedValue xmi:id=\"t1\"><key>{}<value xmi:id=\"k2\" content=\"L\"/></key>\
                 <content><value content=\"V\"/></content></taggedValue>\
                 <taggedValue xmi:id=\"t2\"><key><value content=\"T\"/></key>\
                 <content>{}<value xmi:id=\"c2v\" content=\"b\"/></content></taggedValue>",
                ev("v1", "a", "v2"),
                ev("k1", "K", "k2"),
                ev("c1v", "a", "c2v"),
            ),
        )),
    );
    let input = input.replace(
        "</xmi:XMI>",
        &format!(
            "<sacm:ArtifactPackage xmi:id=\"e\"><name content=\"E\"/>\
             <artifactElement xmi:type=\"sacm:Resource\" xmi:id=\"r1\"><name content=\"R1\"/>\
             <location>{}</location></artifactElement></sacm:ArtifactPackage></xmi:XMI>",
            ev("l1", "u", "l1")
        ),
    );
    let start = writable("expressions naming ids of their own texts", &input);
    for operation in [
        Operation::from(SetContent::remove("d1")),
        SetContent::new("d1", MultiLangString::default()).into(),
        SetContent::new("d1", LangString::new("x")).into(),
        SetKey::new("t1", LangString::new("K")).into(),
        SetTaggedValue::new("c1", "T", LangString::new("x")).into(),
        SetLocation::remove("r1").into(),
        SetLocation::new("r1", MultiLangString::default()).into(),
    ] {
        let mut doc = start.clone();
        accepted(&mut doc, operation.clone());
        still_strict(&format!("expressions of the text, {operation:?}"), &doc);
    }
    // An expression the element still holds after the set refuses it: in
    // the new value, or in another text.
    assert_eq!(
        refused(
            &mut start.clone(),
            SetContent::new("d1", LangString::new("x").with_expression("v2"))
        ),
        part("d1", "expression", "v2"),
        "{ROW}: a new value naming the freed v2"
    );
    let input = in_xmi(
        "",
        "",
        &claim(
            "c1",
            "",
            &format!(
                "<taggedValue xmi:id=\"t1\"><key>{}</key>\
                 <content><value xmi:id=\"tv1\" content=\"a\"/><value xmi:id=\"tv2\" content=\"b\"/>\
                 </content></taggedValue>",
                ev("t1k", "S", "tv2")
            ),
        ),
    );
    let start = writable("a key naming a content value", &input);
    assert_eq!(
        refused(
            &mut start.clone(),
            SetContent::new("t1", LangString::new("a"))
        ),
        part("t1", "expression", "tv2"),
        "{ROW}: the key naming the freed content value tv2"
    );
    // Another end of the edited element, its own citation of v2, still
    // counts.
    let input = in_xmi(
        "",
        "",
        &claim(
            "c1",
            "",
            "<description xmi:id=\"d1\" isCitation=\"true\" citedElement=\"v2\"><content>\
             <value xmi:id=\"v1\" content=\"a\"/><value xmi:id=\"v2\" content=\"b\"/></content>\
             </description>",
        ),
    );
    let start = writable("a description citing its own value", &input);
    assert_eq!(
        refused(
            &mut start.clone(),
            SetContent::new("d1", LangString::new("x"))
        ),
        part("d1", "citedElement", "v2"),
        "{ROW}: the description citing its freed value v2"
    );
    // The name names its own location value lv, which the remove frees.
    let input = format!(
        "<sacm:ArtifactPackage{NS} xmi:id=\"e\"><name content=\"E\"/>\
         <artifactElement xmi:type=\"sacm:Resource\" xmi:id=\"r1\">\
         <name xmi:type=\"sacm:ExpressionLangString\" content=\"R1\" expression=\"lv\"/>\
         <location><value xmi:id=\"lv\" content=\"u\"/></location></artifactElement>\
         </sacm:ArtifactPackage>"
    );
    let start = writable("a name naming its location value", &input);
    assert_eq!(
        refused(&mut start.clone(), SetLocation::remove("r1")),
        part("r1", "expression", "lv"),
        "{ROW}: the name naming the freed location value lv"
    );
}

#[test]
fn lib_edit_026_operations_that_remove_no_id_are_never_refused_for_it() {
    // c1's name id c1n is cited by c3 and named by the document's metadata;
    // the id of its name's metadata, c1nx, is cited by c4; c2 cites d1v.
    let input = in_xmi(
        " xmi:idref=\"c1n\"",
        "<xmi:Difference target=\"c1nx d1v\"/>",
        &format!(
            "<argumentElement xmi:type=\"sacm:Claim\" xmi:id=\"c1\"><name xmi:id=\"c1n\" content=\"C1\">\
             <xmi:Extension xmi:id=\"c1nx\"/></name>\
             <description xmi:id=\"d1\"><content><value xmi:id=\"d1v\" content=\"x\"/></content></description>\
             </argumentElement>{}{}{}\
             <argumentElement xmi:type=\"sacm:ArgumentPackage\" xmi:id=\"q\"><name content=\"Q\"/></argumentElement>",
            claim("c2", " isCitation=\"true\" citedElement=\"d1v\"", ""),
            claim("c3", " isCitation=\"true\" citedElement=\"c1n\"", ""),
            claim("c4", " isCitation=\"true\" citedElement=\"c1nx\"", "")
        ),
    );
    let start = writable("names of c1's parts", &input);
    let copy = copy_of(&start, &["q"], "p");
    let operations: Vec<Operation> = vec![
        SetName::new("c1", en("Renamed")).into(),
        SetName::new("c1", LangString::new("Plain")).into(),
        MoveElement::new("c1", "q").into(),
        MoveElement::new("d1", "c2").into(),
        ReorderElements::new("p", ["c4", "c3", "c2", "c1", "q"]).into(),
        SetReferences::new("c3", ReferenceEnd::CitedElement, ["c2"]).into(),
        RemoveReference::new("c3", ReferenceEnd::CitedElement, "c1n").into(),
        AddReference::new("c2", ReferenceEnd::MetaClaim, "c1").into(),
        SetFlag::new("c1", Flag::IsAbstract, true).into(),
        SetGid::new("c1", "g-1").into(),
        CreateClaim::new("p", en("New")).into(),
        copy.into(),
        // A longer content keeps every id.
        SetContent::new("d1", multi(&[en("x"), sv("y")])).into(),
    ];
    for operation in operations {
        let mut doc = start.clone();
        let name = operation.operation_name();
        accepted(&mut doc, operation);
        still_strict(&format!("names of c1's parts, {name}"), &doc);
        if name == "SetName" {
            assert_eq!(
                doc.get("c1")
                    .and_then(SacmElement::name_id)
                    .map(ElementId::as_str),
                Some("c1n"),
                "{ROW}: SetName kept the name's id"
            );
        }
    }
}

// ---------------------------------------------------------------------------
// 7: the delete policies are unchanged
// ---------------------------------------------------------------------------

#[test]
fn lib_edit_026_references_to_deleted_elements_stay_the_policies() {
    // The inference i names c1 as its source.
    let relationship = "<argumentElement xmi:type=\"sacm:AssertedInference\" xmi:id=\"i\" \
                        source=\"c1\" target=\"c2\"><name content=\"I\"/></argumentElement>";
    let elements = claim("c1", "", "") + &claim("c2", "", "") + relationship;
    let start = writable("an inference", &in_xmi("", "", &elements));
    let error = refusal(&mut start.clone(), &DeleteElement::new("c1").into());
    assert_eq!(
        policy_refusal(&error),
        Some(("Referenced", "i".to_owned(), "source", "c1".to_owned())),
        "{ROW}: RejectIfReferenced: {error:?}"
    );
    for policy in [
        ReferenceDeletePolicy::DeleteReferencingRelationships,
        ReferenceDeletePolicy::ScrubReferences,
    ] {
        let mut doc = start.clone();
        let mutation = accepted(
            &mut doc,
            DeleteElement::new("c1").with_reference_policy(policy),
        );
        assert_eq!(
            mutation.deleted_ids(),
            [ElementId::from("c1"), ElementId::from("i")],
            "{ROW}: {policy:?} deletes the inference"
        );
        still_strict(&format!("{policy:?}"), &doc);
    }
    // A citation of c1 from another package: CrossPackageReference by
    // default, scrubbed otherwise.
    let input = format!(
        "<xmi:XMI{NS}><sacm:ArgumentPackage xmi:id=\"p\"><name content=\"P\"/>{}</sacm:ArgumentPackage>\
         <sacm:ArgumentPackage xmi:id=\"q\"><name content=\"Q\"/>{}</sacm:ArgumentPackage></xmi:XMI>",
        claim("c1", "", ""),
        claim("c9", " isCitation=\"true\" citedElement=\"c1\"", "")
    );
    let start = writable("a crossing citation", &input);
    let error = refusal(&mut start.clone(), &DeleteElement::new("c1").into());
    assert_eq!(
        policy_refusal(&error),
        Some((
            "CrossPackageReference",
            "c9".to_owned(),
            "citedElement",
            "c1".to_owned()
        )),
        "{ROW}: a crossing citation: {error:?}"
    );
    let mut doc = start.clone();
    accepted(&mut doc, scrubbing("c1"));
    still_strict("a crossing citation, scrubbed", &doc);
    // Metadata naming the deleted element: the policies' refusal comes
    // first; under the other policies, metadata refuses too.
    let start = writable(
        "an inference and xmi:XMI naming c1",
        &in_xmi(" xmi:idref=\"c1\"", "", &elements),
    );
    for delete in every_policy("c1") {
        let mut doc = start.clone();
        let error = refusal(&mut doc, &delete.clone().into());
        let policy = format!("{delete:?}");
        if policy.contains("RejectIfReferenced") {
            assert_eq!(
                policy_refusal(&error),
                Some(("Referenced", "i".to_owned(), "source", "c1".to_owned())),
                "{ROW}: {policy}: {error:?}"
            );
        } else {
            assert_eq!(
                named(&error),
                Some(meta(None, "xmi:XMI", "xmi:idref", "c1")),
                "{ROW}: {policy}: {error:?}"
            );
        }
    }
}

// ---------------------------------------------------------------------------
// 8: copies
// ---------------------------------------------------------------------------

/// The source of the copies: in `sp`, the Claim `s1`, whose metadata names
/// the Claim `s2`, and `s2`.
fn copy_source() -> Document {
    writable(
        "the copy source",
        &format!(
            "<sacm:ArgumentPackage{NS} xmi:id=\"sp\"><name content=\"S\"/>{}{}</sacm:ArgumentPackage>",
            claim("s1", "", "<xmi:Extension xmi:idref=\"s2\"/>"),
            claim("s2", "", "")
        ),
    )
}

/// A copy target: the package `q`, holding the Claims `taken`.
fn copy_target(taken: &[&str]) -> Document {
    let claims: String = taken.iter().map(|id| claim(id, "", "")).collect();
    writable(
        "a copy target",
        &format!(
            "<sacm:ArgumentPackage{NS} xmi:id=\"q\"><name content=\"Q\"/>{claims}</sacm:ArgumentPackage>"
        ),
    )
}

/// The copy of `elements` of `source` under `parent`.
fn copy_of(source: &Document, elements: &[&str], parent: &str) -> CopyElements {
    CopyElements::new(source, elements.iter().copied(), parent)
        .unwrap_or_else(|error| panic!("{ROW}: setup: the copy is not built: {error}"))
}

#[test]
fn lib_edit_026_a_copy_refuses_metadata_naming_an_id_the_target_lacks() {
    let source = copy_source();
    // s1 alone, keeping its id: its metadata names s2, which the target lacks.
    assert_eq!(
        refused(&mut copy_target(&[]), copy_of(&source, &["s1"], "q")),
        meta(Some("s1"), "xmi:Extension", "xmi:idref", "s2"),
        "{ROW}: s1 copied alone"
    );
    // s1 alone, under a generated id: the refusal names the copy's id, the
    // one a copy of s1 without its metadata gets.
    let twin = writable(
        "the copy source without metadata",
        &format!(
            "<sacm:ArgumentPackage{NS} xmi:id=\"sp\"><name content=\"S\"/>{}</sacm:ArgumentPackage>",
            claim("s1", "", "")
        ),
    );
    let mut probe = copy_target(&["s1"]);
    let mutation = accepted(&mut probe, copy_of(&twin, &["s1"], "q"));
    let Some(generated) = mutation.created_ids().first().map(ElementId::as_str) else {
        panic!("{ROW}: setup: the twin copy created nothing");
    };
    assert_ne!(generated, "s1", "{ROW}: setup: the copy kept a taken id");
    assert_eq!(
        refused(&mut copy_target(&["s1"]), copy_of(&source, &["s1"], "q")),
        meta(Some(generated), "xmi:Extension", "xmi:idref", "s2"),
        "{ROW}: s1 copied alone under a generated id"
    );
    // Accepted: the token names a copied element keeping its id (whatever
    // id s1 gets), or an element of the target.
    for (what, mut target, copy) in [
        (
            "s1 with s2",
            copy_target(&[]),
            copy_of(&source, &["s1", "s2"], "q"),
        ),
        (
            "s1 with s2 into a target holding s1",
            copy_target(&["s1"]),
            copy_of(&source, &["s1", "s2"], "q"),
        ),
        (
            "s1 into a target holding s2",
            copy_target(&["s2"]),
            copy_of(&source, &["s1"], "q"),
        ),
        (
            "s1 within its own document",
            source.clone(),
            copy_of(&source, &["s1"], "sp"),
        ),
    ] {
        accepted(&mut target, copy);
        still_strict(what, &target);
    }
    // A token naming a part id of the target is accepted too.
    let mut target = writable(
        "a copy target with the part id s2",
        &format!(
            "<sacm:ArgumentPackage{NS} xmi:id=\"q\"><name xmi:id=\"s2\" content=\"Q\"/></sacm:ArgumentPackage>"
        ),
    );
    accepted(&mut target, copy_of(&source, &["s1"], "q"));
    still_strict("s1 into a target whose name has the id s2", &target);
    copies_count_what_they_bring();
}

/// A copy's metadata may name the ids the copy brings: its metadata's, its
/// parts' and its preserved content's; and every token of an `IDREFS` is
/// checked.
fn copies_count_what_they_bring() {
    // s1's metadata names its own metadata id x1 and its name's id s1n.
    let source = writable(
        "a copy source naming its own ids",
        &format!(
            "<sacm:ArgumentPackage{NS} xmi:id=\"sp\"><name content=\"S\"/>\
             <argumentElement xmi:type=\"sacm:Claim\" xmi:id=\"s1\"><name xmi:id=\"s1n\" content=\"S1\"/>\
             <xmi:Extension xmi:id=\"x1\"/><xmi:Extension xmi:idref=\"x1\"/>\
             <xmi:Extension xmi:idref=\"s1n\"/></argumentElement></sacm:ArgumentPackage>"
        ),
    );
    let mut target = copy_target(&[]);
    accepted(&mut target, copy_of(&source, &["s1"], "q"));
    still_strict("s1 naming its own metadata and name ids", &target);
    // s1 holds preserved content with the id px, which its metadata names.
    let source = read_in(
        "a copy source naming its preserved content",
        Mode::Tolerant,
        &format!(
            "<sacm:ArgumentPackage{NS} xmlns:v=\"urn:example:v\" xmi:id=\"sp\"><name content=\"S\"/>{}\
             </sacm:ArgumentPackage>",
            claim(
                "s1",
                "",
                "<v:x xmi:id=\"px\"/><xmi:Extension xmi:idref=\"px\"/>"
            )
        ),
    );
    assert!(
        source.has_preserved_content(),
        "{ROW}: setup: no preserved content"
    );
    accepted(&mut copy_target(&[]), copy_of(&source, &["s1"], "q"));
    // Every token of an IDREFS is checked, not only the first.
    let source = writable(
        "a copy source with an IDREFS",
        &format!(
            "<sacm:ArgumentPackage{NS} xmi:id=\"sp\"><name content=\"S\"/>{}{}</sacm:ArgumentPackage>",
            claim(
                "s3",
                "",
                "<xmi:Extension><xmi:Difference target=\"s3 s2\"/></xmi:Extension>"
            ),
            claim("s2", "", "")
        ),
    );
    assert_eq!(
        refused(&mut copy_target(&[]), copy_of(&source, &["s3"], "q")),
        meta(Some("s3"), "xmi:Difference", "target", "s2"),
        "{ROW}: s3 copied alone"
    );
}

// ---------------------------------------------------------------------------
// 9: the order of the checks and the first offender
// ---------------------------------------------------------------------------

/// A value that is an `ExpressionLangString`, with the expression
/// `expression` or none.
fn expression_value(expression: Option<&str>) -> LangString {
    let input = format!(
        "<sacm:ArgumentPackage{NS} xmi:id=\"x\">\
         <name xmi:type=\"sacm:ExpressionLangString\" content=\"x\"/></sacm:ArgumentPackage>"
    );
    let doc = read_in(
        "an ExpressionLangString with no expression",
        Mode::Tolerant,
        &input,
    );
    let name = doc
        .get("x")
        .and_then(|package| package.name().cloned())
        .unwrap_or_else(|| panic!("{ROW}: setup: the name with no expression is not read"));
    assert!(
        name.is_expression_lang_string() && name.expression().is_none(),
        "{ROW}: setup: not an ExpressionLangString with no expression: {name:?}"
    );
    match expression {
        Some(expression) => name.with_expression(expression),
        None => name,
    }
}

#[test]
#[expect(
    clippy::too_many_lines,
    reason = "the check order of deletes, sets and copies reads best as one table"
)]
fn lib_edit_026_the_checks_come_after_the_others_and_before_the_text_check() {
    let start = fixture();
    let bad = || LangString::new("bad\u{1}");
    // (what, operation, the variant that wins)
    let cases: Vec<(&str, Operation, &str)> = vec![
        // Deletes: checks 1 to 4 first, then PartReference, then
        // MetadataReference.
        (
            "ElementNotFound",
            DeleteElement::new(NOWHERE).into(),
            "ElementNotFound",
        ),
        (
            "NotEmpty (p holds c1)",
            DeleteElement::new("p").into(),
            "NotEmpty",
        ),
        (
            "PartReference before MetadataReference (c1)",
            DeleteElement::new("c1").into(),
            "PartReference",
        ),
        // Sets: the element, the feature and the ExpressionLangStrings first,
        // then the ids, then the text.
        (
            "ElementNotFound",
            SetContent::remove(NOWHERE).into(),
            "ElementNotFound",
        ),
        (
            "FeatureNotApplicable",
            SetContent::remove("c1").into(),
            "FeatureNotApplicable",
        ),
        (
            "FeatureNotApplicable (SetLocation on d1)",
            SetLocation::remove("d1").into(),
            "FeatureNotApplicable",
        ),
        (
            "TooFewReferences",
            SetContent::new("d1", expression_value(None)).into(),
            "TooFewReferences",
        ),
        (
            "ReferenceNotFound",
            SetContent::new("d1", expression_value(Some(NOWHERE))).into(),
            "ReferenceNotFound",
        ),
        (
            "WrongReferenceType",
            SetContent::new("d1", expression_value(Some("c4"))).into(),
            "WrongReferenceType",
        ),
        (
            "PartReference before MetadataReference (d1 removed)",
            SetContent::remove("d1").into(),
            "PartReference",
        ),
        (
            "PartReference before MetadataReference (d1 emptied)",
            SetContent::new("d1", MultiLangString::default()).into(),
            "PartReference",
        ),
        (
            "MetadataReference (SetTaggedValue)",
            SetTaggedValue::new("c1", "ASIL", MultiLangString::default()).into(),
            "MetadataReference",
        ),
        (
            "PartReference (SetLocation)",
            SetLocation::new("r1", MultiLangString::default()).into(),
            "PartReference",
        ),
    ];
    for (what, operation, expected) in cases {
        let error = refusal(&mut start.clone(), &operation);
        assert_eq!(
            variant(&error),
            expected,
            "{ROW}: {what}: {expected} does not win: {error:?}"
        );
    }
    // Delete check 3 (the policies) comes first: c1 is the source of i, its
    // value is cited by c2 and xmi:XMI names it.
    let input = in_xmi(
        " xmi:idref=\"c1\"",
        "",
        &(claim(
            "c1",
            "",
            "<description xmi:id=\"d1\"><content><value xmi:id=\"d1v\" content=\"x\"/></content></description>",
        ) + &claim("c2", " isCitation=\"true\" citedElement=\"d1v\"", "")
            + "<argumentElement xmi:type=\"sacm:AssertedInference\" xmi:id=\"i\" source=\"c1\" target=\"c2\">\
               <name content=\"I\"/></argumentElement>"),
    );
    let start = writable("c1 referenced three ways", &input);
    let error = refusal(&mut start.clone(), &DeleteElement::new("c1").into());
    assert_eq!(
        variant(&error),
        "Referenced",
        "{ROW}: Referenced does not win: {error:?}"
    );
    // Delete check 4: p2 is implemented by pi, which holds a Claim and would
    // go for its bound; p2's name id is cited and xmi:XMI names p2.
    let input = format!(
        "<xmi:XMI{NS} xmi:idref=\"p2\"><sacm:AssuranceCasePackage xmi:id=\"case\"><name content=\"Case\"/>\
         <argumentPackage xmi:id=\"p2\"><name xmi:id=\"p2n\" content=\"P2\"/></argumentPackage>\
         <argumentPackage xmi:type=\"sacm:ArgumentPackageInterface\" xmi:id=\"pi\" implements=\"p2\">\
         <name content=\"PI\"/>{}</argumentPackage>\
         <argumentPackage xmi:id=\"p\"><name content=\"P\"/>{}</argumentPackage>\
         </sacm:AssuranceCasePackage></xmi:XMI>",
        claim("ci", "", ""),
        claim("c", " isCitation=\"true\" citedElement=\"p2n\"", "")
    );
    let start = writable("an interface removed for its bound", &input);
    let error = refusal(&mut start.clone(), &scrubbing("p2").into());
    assert_eq!(
        (variant(&error), format!("{error:?}").contains("\"pi\"")),
        ("NotEmpty".to_owned(), true),
        "{ROW}: NotEmpty naming pi does not win: {error:?}"
    );
    // A copy: checks 1 to 6 first.
    let source = copy_source();
    let dangling_source = writable(
        "a copy source with a dangling citation",
        &format!(
            "<sacm:ArgumentPackage{NS} xmi:id=\"sp\"><name content=\"S\"/>{}{}</sacm:ArgumentPackage>",
            claim(
                "s1",
                " isCitation=\"true\" citedElement=\"s3\"",
                "<xmi:Extension xmi:idref=\"s2\"/>"
            ),
            claim("s2", "", "") + &claim("s3", "", "")
        ),
    );
    let copies: Vec<(&str, CopyElements, &str)> = vec![
        (
            "ParentNotFound",
            copy_of(&source, &["s1"], NOWHERE),
            "ParentNotFound",
        ),
        (
            "InvalidParent",
            copy_of(&source, &["s1"], "t"),
            "InvalidParent",
        ),
        (
            "ReferenceNotFound",
            copy_of(&dangling_source, &["s1"], "q"),
            "ReferenceNotFound",
        ),
        // s2 is taken, so its copy changes its id, which s1's metadata names.
        (
            "DuplicateId",
            copy_of(&source, &["s1", "s2"], "q"),
            "DuplicateId",
        ),
    ];
    let target = copy_target(&["t", "s2"]);
    for (what, copy, expected) in copies {
        let error = refusal(&mut target.clone(), &copy.into());
        assert_eq!(
            variant(&error),
            expected,
            "{ROW}: a copy, {what}: {expected} does not win: {error:?}"
        );
    }
    // The ids come before LIB-EDIT-025's text check: one bad value frees
    // d1v2, which d1's metadata names.
    let start = fixture();
    let error = refusal(&mut start.clone(), &SetContent::new("d1", bad()).into());
    assert_eq!(
        variant(&error),
        "MetadataReference",
        "{ROW}: MetadataReference does not win over UnrepresentableCharacter: {error:?}"
    );
    // The text check stays last: two values keep both ids, one is bad.
    let error = refusal(
        &mut start.clone(),
        &SetContent::new("d1", multi(&[en("ok"), bad()])).into(),
    );
    assert_eq!(
        variant(&error),
        "UnrepresentableCharacter",
        "{ROW}: a set keeping every id is not refused for its text: {error:?}"
    );
}

#[test]
#[expect(
    clippy::too_many_lines,
    reason = "the order rules read best as one table of cases"
)]
fn lib_edit_026_the_first_offender_is_named_in_document_order() {
    // Typed references: by holder in pre-order, then by end in
    // ReferenceEnd::ALL order, then by position.
    let values = "<description xmi:id=\"d1\"><content><value xmi:id=\"v1\" lang=\"en\" content=\"a\"/>\
                  <value xmi:id=\"v2\" lang=\"sv\" content=\"b\"/></content></description>";
    let cases = [
        (
            "the holder first in pre-order",
            claim("h1", " metaClaim=\"v2 v1\"", "")
                + &claim("c1", "", values)
                + &claim("h2", " isCitation=\"true\" citedElement=\"v1\"", ""),
            part("h1", "metaClaim", "v2"),
        ),
        (
            "the end first in ReferenceEnd::ALL",
            claim("c1", "", values)
                + &claim(
                    "h1",
                    " metaClaim=\"v1\" isCitation=\"true\" citedElement=\"v2\"",
                    "",
                ),
            part("h1", "citedElement", "v2"),
        ),
        (
            "the position first",
            claim("c1", "", values) + &claim("h1", " metaClaim=\"c1 v2 v1\"", ""),
            part("h1", "metaClaim", "v2"),
        ),
    ];
    for (what, elements, expected) in cases {
        let start = writable(what, &in_xmi("", "", &elements));
        for operation in [
            Operation::from(SetContent::remove("d1")),
            DeleteElement::new("d1").into(),
        ] {
            assert_eq!(
                refused(&mut start.clone(), operation.clone()),
                expected,
                "{ROW}: {what}, {operation:?}"
            );
        }
    }
    // Metadata: the document's first, the xmi:XMI attributes before its
    // fragments; then each remaining element in pre-order, its own metadata,
    // its name's, then its texts' in the order they are written (content,
    // key, location) with their values; within a fragment its events, then
    // attributes, then tokens, in order.
    let base = |holders: &str| claim("c1", "", "<description xmi:id=\"d1\"/>") + holders;
    let x = |attributes: &str| format!("<xmi:Extension{attributes}/>");
    let cases = [
        (
            "xmi:XMI before the fragments",
            in_xmi(
                " xmi:idref=\"c1\"",
                "<xmi:Difference target=\"c1\"/>",
                &base(&claim("c2", "", &x(" xmi:idref=\"c1\""))),
            ),
            meta(None, "xmi:XMI", "xmi:idref", "c1"),
        ),
        (
            "the document before the elements",
            in_xmi(
                "",
                "<xmi:Difference target=\"c1\"/><xmi:Extension xmi:idref=\"c1\"/>",
                &base(&claim("c2", "", &x(" xmi:idref=\"c1\""))),
            ),
            meta(None, "xmi:Difference", "target", "c1"),
        ),
        (
            "the elements in pre-order",
            in_xmi(
                "",
                "",
                &(claim(
                    "c3",
                    "",
                    "<xmi:Extension><xmi:Delete target=\"c1\"/></xmi:Extension>",
                ) + &base(&claim("c2", "", &x(" xmi:idref=\"c1\"")))),
            ),
            meta(Some("c3"), "xmi:Delete", "target", "c1"),
        ),
        (
            "an element's own metadata before its name's",
            in_xmi(
                "",
                "",
                &base(
                    "<argumentElement xmi:type=\"sacm:Claim\" xmi:id=\"c2\"><name content=\"C2\">\
                     <xmi:Extension xmi:idref=\"c1\"/></name>\
                     <xmi:Extension><xmi:Delete target=\"c1\"/></xmi:Extension></argumentElement>",
                ),
            ),
            meta(Some("c2"), "xmi:Delete", "target", "c1"),
        ),
        (
            "a content before a key, as they are written",
            in_xmi(
                "",
                "",
                &base(&claim(
                    "c2",
                    "",
                    "<taggedValue xmi:id=\"t2\"><key><xmi:Extension xmi:idref=\"c1\"/><value content=\"k\"/></key>\
                     <content><xmi:Extension><xmi:Delete target=\"c1\"/></xmi:Extension><value content=\"v\"/>\
                     </content></taggedValue>",
                )),
            ),
            meta(Some("t2"), "xmi:Delete", "target", "c1"),
        ),
        (
            "a text before its values",
            in_xmi(
                "",
                "",
                &base(&claim(
                    "c2",
                    "",
                    "<note xmi:id=\"n2\"><content><value content=\"v\"><xmi:Extension xmi:idref=\"c1\"/></value>\
                     <xmi:Extension><xmi:Delete target=\"c1\"/></xmi:Extension></content></note>",
                )),
            ),
            meta(Some("n2"), "xmi:Delete", "target", "c1"),
        ),
        (
            "the events of a fragment in order",
            in_xmi(
                "",
                "<xmi:Extension xmi:idref=\"c1\"><xmi:Difference target=\"c1\"/></xmi:Extension>",
                &base(""),
            ),
            meta(None, "xmi:Extension", "xmi:idref", "c1"),
        ),
        (
            "the attributes in order",
            in_xmi(
                "",
                "<xmi:Difference container=\"c1\" target=\"c1\"/>",
                &base(""),
            ),
            meta(None, "xmi:Difference", "container", "c1"),
        ),
        (
            "the tokens in order",
            in_xmi("", "<xmi:Difference target=\"p d1 c1\"/>", &base("")),
            meta(None, "xmi:Difference", "target", "d1"),
        ),
    ];
    for (what, input, expected) in cases {
        let start = writable(what, &input);
        assert_eq!(
            refused(&mut start.clone(), DeleteElement::new("c1")),
            expected,
            "{ROW}: {what}"
        );
    }
    // A typed reference before metadata, for a delete and for a set.
    let start = fixture();
    assert_eq!(
        refused(&mut start.clone(), DeleteElement::new("c1")),
        part("c2", "citedElement", "d1v1"),
        "{ROW}: the fixture, c1's delete"
    );
    assert_eq!(
        refused(&mut start.clone(), SetContent::remove("d1")),
        part("c2", "citedElement", "d1v1"),
        "{ROW}: the fixture, d1's content removed"
    );
}

// ---------------------------------------------------------------------------
// 10: check, preview, apply and the batches agree
// ---------------------------------------------------------------------------

/// One refusal of each kind: (the document, the operation).
fn sample_refusals() -> Vec<(&'static str, Document, Operation)> {
    vec![
        ("probe 1", probe_one(), DeleteElement::new("c1").into()),
        ("probe 2", probe_two(), SetContent::remove("d1").into()),
        ("probe 2", probe_two(), DeleteElement::new("d1").into()),
        ("the fixture", fixture(), DeleteElement::new("t1").into()),
        ("the fixture", fixture(), DeleteElement::new("c1").into()),
        (
            "the fixture",
            fixture(),
            SetContent::new("d1", en("Only")).into(),
        ),
        ("the fixture", fixture(), SetLocation::remove("r1").into()),
        (
            "the fixture",
            fixture(),
            SetTaggedValue::new("c1", "ASIL", MultiLangString::default()).into(),
        ),
        (
            "a copy target",
            copy_target(&[]),
            copy_of(&copy_source(), &["s1"], "q").into(),
        ),
    ]
}

#[test]
#[expect(
    clippy::too_many_lines,
    reason = "every entry point is checked on every sample in one loop"
)]
fn lib_edit_026_check_preview_apply_and_batches_agree_and_a_refusal_changes_nothing() {
    for (what, start, operation) in sample_refusals() {
        // A good operation to run first: a Claim in the first package.
        let Some(package) = start
            .elements()
            .find(|element| element.kind() == ElementKind::ArgumentPackage)
            .map(|element| element.id().clone())
        else {
            panic!("{ROW}: setup: {what} has no ArgumentPackage");
        };
        let good =
            || Operation::from(CreateClaim::new(package.clone(), en("Fine")).with_id("fine"));
        let mut doc = start.clone();
        let (revision, bytes) = (doc.revision(), written(&doc));
        let unchanged = |doc: &Document, how: &str| {
            assert!(
                *doc == start && doc.revision() == revision && written(doc) == bytes,
                "{ROW}: {what}: {how} of {operation:?} changed the document, its revision or \
                 what it writes"
            );
        };
        // check, preview, apply and apply_unchecked_text (`refusal`).
        let error = refusal(&mut doc, &operation);
        assert!(
            named(&error).is_some(),
            "{ROW}: {what}: {operation:?} is not refused by LIB-EDIT-026: {error:?}"
        );
        unchanged(&doc, "apply");
        let assert_batch = |answer: Result<_, BatchError>, how: &str| match answer {
            Ok(batch) => panic!("{ROW}: {what}: {how} accepts {operation:?}: {batch:?}"),
            Err(failure) => assert_eq!(
                (failure.index(), failure.error()),
                (1, &error),
                "{ROW}: {what}: {how} does not refuse {operation:?} at index 1 with apply's refusal"
            ),
        };
        assert_batch(
            edit::batch(&mut doc, |tx| -> Result<(), BatchError> {
                tx.apply(good())?;
                tx.apply(operation.clone())?;
                Ok(())
            }),
            "Batch::apply",
        );
        unchanged(&doc, "Batch::apply");
        assert_batch(
            edit::apply_all(&mut doc, [good(), operation.clone()]),
            "apply_all",
        );
        unchanged(&doc, "apply_all");
        assert_batch(
            edit::preview_batch(&doc, |tx| -> Result<(), BatchError> {
                tx.apply(good())?;
                tx.apply(operation.clone())?;
                Ok(())
            }),
            "preview_batch",
        );
        assert_batch(
            edit::preview_all(&doc, [good(), operation.clone()]),
            "preview_all",
        );
        unchanged(&doc, "preview_batch and preview_all");
    }
    // In a batch, removing the reference first lets the content go.
    let start = probe_two();
    let remove = || {
        Operation::from(RemoveReference::new(
            "c2",
            ReferenceEnd::CitedElement,
            "d1v",
        ))
    };
    let empty = || Operation::from(SetContent::remove("d1"));
    let mut doc = start.clone();
    if let Err(failure) = edit::apply_all(&mut doc, [remove(), empty()]) {
        panic!("{ROW}: RemoveReference then SetContent::remove is refused: {failure:?}");
    }
    still_strict("RemoveReference then SetContent::remove", &doc);
    let mut doc = start.clone();
    if let Err(failure) = edit::batch(&mut doc, |tx| -> Result<(), BatchError> {
        tx.apply(remove())?;
        tx.apply(empty())?;
        Ok(())
    }) {
        panic!("{ROW}: Batch::apply of RemoveReference then SetContent::remove: {failure:?}");
    }
    still_strict(
        "Batch::apply of RemoveReference then SetContent::remove",
        &doc,
    );
    // The other order is refused at the content, and changes nothing.
    let mut doc = start.clone();
    match edit::apply_all(&mut doc, [empty(), remove()]) {
        Ok(batch) => {
            panic!("{ROW}: SetContent::remove then RemoveReference is accepted: {batch:?}")
        }
        Err(failure) => assert_eq!(
            (failure.index(), named(failure.error())),
            (0, Some(part("c2", "citedElement", "d1v"))),
            "{ROW}: SetContent::remove then RemoveReference"
        ),
    }
    assert!(
        doc == start,
        "{ROW}: the refused batch changed the document"
    );
}

// ---------------------------------------------------------------------------
// 11: the code, the message and the JSON
// ---------------------------------------------------------------------------

/// The cells of the catalog row of `code`, which must be the only one.
fn catalog_row(code: &str) -> Vec<String> {
    let path = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../..")
        .join(CATALOG);
    let catalog = std::fs::read_to_string(&path)
        .unwrap_or_else(|error| panic!("{ROW}: setup: read {}: {error}", path.display()));
    let rows: Vec<Vec<String>> = catalog
        .lines()
        .filter_map(|line| line.trim().strip_prefix('|'))
        .map(|line| line.split('|').map(|cell| cell.trim().to_owned()).collect())
        .filter(|cells: &Vec<String>| cells.first().is_some_and(|first| first == code))
        .collect();
    match <[Vec<String>; 1]>::try_from(rows) {
        Ok([row]) => row,
        Err(rows) => panic!(
            "{ROW}: {CATALOG} has {} rows for {code}, not one",
            rows.len()
        ),
    }
}

/// The three refusals of the golden samples: probe 2's content removed,
/// probe 1's c1 deleted, and the fixture's t1 deleted.
fn golden_refusals() -> [EditError; 3] {
    [
        refusal(&mut probe_two(), &SetContent::remove("d1").into()),
        refusal(&mut probe_one(), &DeleteElement::new("c1").into()),
        refusal(&mut fixture(), &DeleteElement::new("t1").into()),
    ]
}

#[test]
fn lib_edit_026_the_refusals_have_code_sacm_ref_001_and_the_catalog_names_them() {
    for error in golden_refusals() {
        assert_eq!(
            (error.code(), error.code().as_str()),
            (Code::REF_DANGLING, "SACM-REF-001"),
            "{ROW}: the code of {error:?}"
        );
        let as_error: &dyn std::error::Error = &error;
        assert!(as_error.source().is_none(), "{ROW}: {error:?} has a source");
    }
    let row = catalog_row("SACM-REF-001");
    let (severity, meaning) = (
        row.get(1).map_or("", String::as_str),
        row.get(2).map_or("", String::as_str),
    );
    assert_eq!(severity, "Error", "{ROW}: the severity of SACM-REF-001");
    for needle in [
        "`EditError::PartReference`",
        "`EditError::MetadataReference`",
        "`CopyElements`",
        "LIB-EDIT-026",
    ] {
        assert!(
            meaning.contains(needle),
            "{ROW}: the {CATALOG} row of SACM-REF-001 does not name {needle}"
        );
    }
}

#[test]
fn lib_edit_026_display_and_json_match_the_golden_samples() {
    let refusals = golden_refusals();
    assert_eq!(
        refusals.each_ref().map(named),
        [
            Some(part("c2", "citedElement", "d1v")),
            Some(meta(None, "xmi:XMI", "xmi:idref", "c1")),
            Some(meta(Some("c2"), "xmi:Extension", "xmi:idref", "t1v")),
        ],
        "{ROW}: the golden refusals"
    );
    assert_eq!(
        refusals.each_ref().map(ToString::to_string),
        [
            "the citedElement of \"c2\" names \"d1v\", the xmi:id of a name or text, and after the \
             edit the document would have no such xmi:id",
            "the attribute \"xmi:idref\" of the element \"xmi:XMI\", kept as XMI metadata, names \
             \"c1\", and after the edit the document would have no such xmi:id",
            "the attribute \"xmi:idref\" of the element \"xmi:Extension\", kept as XMI metadata of \
             \"c2\", names \"t1v\", and after the edit the document would have no such xmi:id",
        ],
        "{ROW}: the Display of the golden refusals"
    );
    assert_json(&refusals);
}

/// The compact JSON of the three golden refusals.
#[cfg(feature = "serde")]
const GOLDEN_JSON: [&str; 3] = [
    r#"{"type":"PartReference","code":"SACM-REF-001","holder":"c2","end":"citedElement","id":"d1v"}"#,
    r#"{"type":"MetadataReference","code":"SACM-REF-001","holder":null,"element":"xmi:XMI","attribute":"xmi:idref","id":"c1"}"#,
    r#"{"type":"MetadataReference","code":"SACM-REF-001","holder":"c2","element":"xmi:Extension","attribute":"xmi:idref","id":"t1v"}"#,
];

/// With `serde`, each refusal serializes as its sample, which reads back
/// equal; `edit-errors.json` holds the three, in order, from its
/// `PartReference` sample; a wrong code, an unknown end and each missing key
/// are refused as data.
#[cfg(feature = "serde")]
fn assert_json(refusals: &[EditError; 3]) {
    for (error, sample) in refusals.iter().zip(GOLDEN_JSON) {
        let written = serde_json::to_string(error)
            .unwrap_or_else(|failure| panic!("{ROW}: {error:?} does not serialize: {failure}"));
        assert_eq!(written, sample, "{ROW}: the JSON of {error:?}");
        let read: EditError = serde_json::from_str(sample)
            .unwrap_or_else(|failure| panic!("{ROW}: {sample} does not read: {failure}"));
        assert_eq!(&read, error, "{ROW}: {sample} does not read back equal");
        let mut changes = vec![
            (
                "a wrong code".to_owned(),
                sample.replacen("\"SACM-REF-001\"", "\"SACM-CMD-001\"", 1),
            ),
            (
                "another SACM-REF code".to_owned(),
                sample.replacen("\"SACM-REF-001\"", "\"SACM-REF-002\"", 1),
            ),
            (
                "an unknown end".to_owned(),
                sample.replacen("\"citedElement\"", "\"citation\"", 1),
            ),
        ];
        let object: serde_json::Map<String, serde_json::Value> = serde_json::from_str(sample)
            .unwrap_or_else(|failure| panic!("{ROW}: setup: {sample}: {failure}"));
        for key in object.keys().filter(|key| *key != "type") {
            let mut fewer = object.clone();
            fewer.remove(key);
            let text = serde_json::to_string(&fewer)
                .unwrap_or_else(|failure| panic!("{ROW}: setup: {failure}"));
            changes.push((format!("a missing {key}"), text));
        }
        for (what, changed) in changes {
            if changed == sample {
                // A sample without the end has no end to change.
                continue;
            }
            match serde_json::from_str::<EditError>(&changed) {
                Ok(value) => panic!("{ROW}: {what}: {changed} was read as {value:?}"),
                Err(failure) => assert!(
                    failure.is_data(),
                    "{ROW}: {what}: {changed} is refused as malformed JSON: {failure}"
                ),
            }
        }
    }
    let path =
        Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/data/golden/serde/edit-errors.json");
    let golden = std::fs::read_to_string(&path)
        .unwrap_or_else(|failure| panic!("{ROW}: setup: read {}: {failure}", path.display()));
    let samples: Vec<EditError> = serde_json::from_str(&golden)
        .unwrap_or_else(|failure| panic!("{ROW}: edit-errors.json does not read: {failure}"));
    let Some(first) = samples
        .iter()
        .position(|sample| matches!(named(sample), Some(Named::Part { .. })))
    else {
        panic!("{ROW}: edit-errors.json holds no PartReference sample");
    };
    assert_eq!(
        samples.get(first..first + 3),
        Some(refusals.as_slice()),
        "{ROW}: edit-errors.json does not hold the three refusals, in order"
    );
}

/// Without `serde`, there is no JSON to check.
#[cfg(not(feature = "serde"))]
fn assert_json(_refusals: &[EditError; 3]) {}

#[cfg(feature = "serde")]
#[test]
fn lib_edit_026_an_operation_read_from_json_is_refused_alike() {
    let cases: [(&str, Operation, Named); 3] = [
        (
            r#"{"type":"DeleteElement","element":"t1"}"#,
            DeleteElement::new("t1").into(),
            meta(Some("c2"), "xmi:Extension", "xmi:idref", "t1v"),
        ),
        (
            r#"{"type":"SetContent","element":"d1","content":[{"lang":"en","content":"Only"}]}"#,
            SetContent::new("d1", en("Only")).into(),
            meta(Some("d1"), "xmi:Extension", "xmi:idref", "d1v2"),
        ),
        (
            r#"{"type":"SetLocation","element":"r1","location":null}"#,
            SetLocation::remove("r1").into(),
            part("r2", "citedElement", "r1lv"),
        ),
    ];
    let start = fixture();
    for (json, twin, expected) in cases {
        let operation: Operation = serde_json::from_str(json)
            .unwrap_or_else(|failure| panic!("{ROW}: {json} does not read: {failure}"));
        assert_eq!(operation, twin, "{ROW}: {json} does not read as its twin");
        assert_eq!(
            refused(&mut start.clone(), operation),
            expected,
            "{ROW}: {json}"
        );
        assert_eq!(
            refused(&mut start.clone(), twin),
            expected,
            "{ROW}: the twin of {json}"
        );
    }
    // A list read from JSON, applied with apply_all: refused at index 1,
    // and accepted once the reference goes first.
    let start = probe_two();
    let list = r#"[{"type":"SetGid","element":"c1","gid":"g"},{"type":"SetContent","element":"d1","content":null}]"#;
    let operations: Vec<Operation> = serde_json::from_str(list)
        .unwrap_or_else(|failure| panic!("{ROW}: {list} does not read: {failure}"));
    let mut doc = start.clone();
    match edit::apply_all(&mut doc, operations) {
        Ok(batch) => panic!("{ROW}: apply_all accepts {list}: {batch:?}"),
        Err(failure) => assert_eq!(
            (failure.index(), named(failure.error())),
            (1, Some(part("c2", "citedElement", "d1v"))),
            "{ROW}: apply_all of {list}"
        ),
    }
    assert!(doc == start, "{ROW}: the refused list changed the document");
    let list = r#"[{"type":"RemoveReference","element":"c2","end":"citedElement","id":"d1v"},{"type":"SetContent","element":"d1","content":null}]"#;
    let operations: Vec<Operation> = serde_json::from_str(list)
        .unwrap_or_else(|failure| panic!("{ROW}: {list} does not read: {failure}"));
    if let Err(failure) = edit::apply_all(&mut doc, operations) {
        panic!("{ROW}: apply_all refuses {list}: {failure:?}");
    }
    still_strict(list, &doc);
}

// ---------------------------------------------------------------------------
// 12: accepted edits keep a strictly writable document strictly writable
// ---------------------------------------------------------------------------

/// The random sequences from each input.
const SEEDS: u64 = 16;

/// The operations of each random sequence.
const STEPS: usize = 40;

/// `SplitMix64`, as in `batch.rs`: the same on every platform and run.
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

    /// No, one or two values, each with a language half the time.
    fn multi(&mut self) -> MultiLangString {
        const TEXTS: [&str; 4] = ["a", "Säker", "x y", ""];
        const LANGS: [&str; 3] = ["en", "sv", "de"];
        let count = self.below(3);
        (0..count)
            .map(|_| {
                let value = LangString::new(*self.pick(&TEXTS).unwrap_or(&"a"));
                if self.flip() {
                    value.with_lang(*self.pick(&LANGS).unwrap_or(&"en"))
                } else {
                    value
                }
            })
            .collect()
    }

    /// The id of an element of `doc`, of a kind `wanted` admits three times
    /// in four when there is one; [`NOWHERE`] when there is none.
    fn id_of(&mut self, doc: &Document, wanted: impl Fn(ElementKind) -> bool) -> ElementId {
        let of_kind: Vec<ElementId> = doc
            .elements()
            .filter(|element| wanted(element.kind()))
            .map(|element| element.id().clone())
            .collect();
        if !of_kind.is_empty() && self.below(4) != 0 {
            if let Some(id) = self.pick(&of_kind) {
                return id.clone();
            }
        }
        let all: Vec<ElementId> = doc.elements().map(|element| element.id().clone()).collect();
        self.pick(&all)
            .cloned()
            .unwrap_or_else(|| ElementId::from(NOWHERE))
    }
}

/// Whether `kind` holds a `content`: a `UtilityElement`.
fn has_content(kind: ElementKind) -> bool {
    matches!(
        kind,
        ElementKind::Description
            | ElementKind::ImplementationConstraint
            | ElementKind::Note
            | ElementKind::TaggedValue
    )
}

/// One random operation for the state `doc`: a delete under one of the 12
/// policy combinations, a text set, a `SetName`, a move, a copy (from `doc`
/// or `source`), a `RemoveReference` or an `AddReference`. `None` when a copy
/// cannot be built.
fn random_operation(rng: &mut Rng, doc: &Document, source: &Document) -> Option<Operation> {
    let operation = match rng.below(16) {
        0..=3 => {
            let deletes = every_policy(rng.id_of(doc, |_| true).as_str());
            rng.pick(&deletes)?.clone().into()
        }
        4..=9 => random_text_set(rng, doc),
        10 => SetName::new(rng.id_of(doc, |_| true), en("Renamed")).into(),
        11 => {
            let element = rng.id_of(doc, |_| true);
            if rng.below(4) == 0 {
                MoveElement::root(element).into()
            } else {
                MoveElement::new(element, rng.id_of(doc, |_| true)).into()
            }
        }
        12 => {
            let from = if rng.flip() { doc } else { source };
            let element = rng.id_of(from, |_| true);
            CopyElements::new(from, [element], rng.id_of(doc, |_| true))
                .ok()?
                .into()
        }
        13 | 14 => {
            // A reference a random element holds.
            let held: Vec<(ElementId, ReferenceEnd, ElementId)> = doc
                .elements()
                .flat_map(|element| {
                    ReferenceEnd::ALL
                        .iter()
                        .filter(|end| **end != ReferenceEnd::Expression)
                        .flat_map(move |&end| {
                            element
                                .references(end)
                                .iter()
                                .map(move |id| (element.id().clone(), end, id.clone()))
                        })
                })
                .collect();
            let (holder, end, id) = rng.pick(&held)?.clone();
            RemoveReference::new(holder, end, id).into()
        }
        _ => {
            let ends: Vec<ReferenceEnd> = ReferenceEnd::ALL
                .iter()
                .copied()
                .filter(|end| *end != ReferenceEnd::Expression)
                .collect();
            let end = *rng.pick(&ends)?;
            let holder = rng.id_of(doc, |kind| end.applies_to(kind));
            AddReference::new(holder, end, rng.id_of(doc, |_| true)).into()
        }
    };
    Some(operation)
}

/// One random text set: `SetContent`, `SetKey`, `SetLocation` or
/// `SetTaggedValue`, most often on an element with the feature, removing a
/// quarter of the time.
fn random_text_set(rng: &mut Rng, doc: &Document) -> Operation {
    let remove = rng.below(4) == 0;
    match rng.below(4) {
        0 => {
            let target = rng.id_of(doc, has_content);
            if remove {
                SetContent::remove(target).into()
            } else {
                SetContent::new(target, rng.multi()).into()
            }
        }
        1 => SetKey::new(
            rng.id_of(doc, |kind| kind == ElementKind::TaggedValue),
            rng.multi(),
        )
        .into(),
        2 => {
            let target = rng.id_of(doc, |kind| kind == ElementKind::Resource);
            if remove {
                SetLocation::remove(target).into()
            } else {
                SetLocation::new(target, rng.multi()).into()
            }
        }
        _ => {
            // The key of one of its tagged values two times in three, so the
            // set path runs.
            let target = rng.id_of(doc, |kind| kind.may_contain(ElementKind::TaggedValue));
            let held: Option<String> = doc
                .get(&target)
                .and_then(|element| element.tagged_values().next())
                .and_then(SacmElement::key)
                .and_then(|key| key.values().first())
                .and_then(LangString::content)
                .map(str::to_owned);
            let key = match held {
                Some(key) if rng.below(3) != 0 => key,
                _ => "k".to_owned(),
            };
            SetTaggedValue::new(target, key, rng.multi()).into()
        }
    }
}

/// The operations that set a text.
const TEXT_SETS: [&str; 4] = ["SetContent", "SetKey", "SetLocation", "SetTaggedValue"];

/// How often each operation was accepted, and how often each LIB-EDIT-026
/// refusal refused it: `(operation, "accepted" or the variant)`.
type Tally = BTreeMap<(&'static str, &'static str), usize>;

/// Runs [`STEPS`] random operations on `doc` with `seed`. After each
/// accepted one, a strict write succeeds and its output reads back strictly
/// equal, or, once no root is left, refuses with `NoRoot`; each refusal
/// leaves the document `==` to before.
fn random_run(label: &str, mut doc: Document, source: &Document, seed: u64, tally: &mut Tally) {
    let mut rng = Rng(seed);
    for step in 0..STEPS {
        if doc.element_count() == 0 {
            break;
        }
        let Some(operation) = random_operation(&mut rng, &doc, source) else {
            continue;
        };
        let what = format!("{label}, seed {seed}, step {step}, {operation:?}");
        let name = operation.operation_name();
        let before = doc.clone();
        match edit::apply(&mut doc, operation) {
            Ok(_) => *tally.entry((name, "accepted")).or_default() += 1,
            Err(error) => {
                match named(&error) {
                    Some(Named::Part { .. }) => {
                        *tally.entry((name, "PartReference")).or_default() += 1;
                    }
                    Some(Named::Metadata { .. }) => {
                        *tally.entry((name, "MetadataReference")).or_default() += 1;
                    }
                    None => {}
                }
                assert!(
                    doc == before,
                    "{ROW}: {what}: the refusal changed the document"
                );
                continue;
            }
        }
        if doc.roots().next().is_none() {
            assert!(
                matches!(
                    xmi::write(&doc, &WriteOptions::default()),
                    Err(WriteError::NoRoot)
                ),
                "{ROW}: {what}: a document with no root is not refused as NoRoot"
            );
            continue;
        }
        still_strict(&what, &doc);
    }
}

/// Every `*.golden.sacm.xmi` under the test data and every
/// `sacm23/xmi/*-valid.sacm.xmi`, sorted.
fn candidate_inputs() -> Vec<PathBuf> {
    let data = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/data");
    let mut paths = Vec::new();
    let mut pending = vec![data.clone()];
    while let Some(dir) = pending.pop() {
        let entries = std::fs::read_dir(&dir)
            .unwrap_or_else(|error| panic!("{ROW}: setup: list {}: {error}", dir.display()));
        for entry in entries {
            let path = entry
                .unwrap_or_else(|error| panic!("{ROW}: setup: list {}: {error}", dir.display()))
                .path();
            let name = path.to_string_lossy().replace('\\', "/");
            if path.is_dir() {
                pending.push(path);
            } else if name.ends_with(".golden.sacm.xmi")
                || (name.ends_with("-valid.sacm.xmi")
                    && path.parent() == Some(data.join("sacm23/xmi").as_path()))
            {
                paths.push(path);
            }
        }
    }
    paths.sort();
    paths
}

#[test]
fn lib_edit_026_accepted_edits_keep_a_strictly_writable_document_strictly_writable() {
    // The inputs: those a strict read and a strict write accept.
    let mut inputs: Vec<(String, Document)> = Vec::new();
    for path in candidate_inputs() {
        let bytes = std::fs::read(&path)
            .unwrap_or_else(|error| panic!("{ROW}: setup: read {}: {error}", path.display()));
        let Ok(output) = xmi::read(&bytes, &ReadOptions::default()) else {
            continue;
        };
        let doc = output.into_document();
        if xmi::write(&doc, &WriteOptions::default()).is_err() {
            continue;
        }
        let label = path
            .file_name()
            .map_or_else(String::new, |name| name.to_string_lossy().into_owned());
        still_strict(&format!("setup: {label}"), &doc);
        inputs.push((label, doc));
    }
    assert!(
        inputs.len() >= 20,
        "{ROW}: setup: only {} inputs a strict read and a strict write accept",
        inputs.len()
    );
    assert!(
        inputs
            .iter()
            .any(|(label, _)| label == "named-ids-valid.sacm.xmi"),
        "{ROW}: setup: the fixture is not an input"
    );
    let source = fixture();
    let mut tally = Tally::new();
    for (label, doc) in &inputs {
        for seed in 0..SEEDS {
            random_run(label, doc.clone(), &source, seed, &mut tally);
        }
    }
    // Every kind of operation was accepted.
    for operation in [
        "DeleteElement",
        "SetContent",
        "SetKey",
        "SetLocation",
        "SetTaggedValue",
        "SetName",
        "MoveElement",
        "CopyElements",
        "RemoveReference",
        "AddReference",
    ] {
        assert!(
            tally.get(&(operation, "accepted")).copied().unwrap_or(0) > 0,
            "{ROW}: setup: the sequences accept no {operation}: {tally:?}"
        );
    }
    // Both refusals were hit by a delete and by a text set.
    for refusal in ["PartReference", "MetadataReference"] {
        assert!(
            tally.get(&("DeleteElement", refusal)).copied().unwrap_or(0) > 0,
            "{ROW}: setup: no delete is refused as {refusal}: {tally:?}"
        );
        let by_sets: usize = TEXT_SETS
            .iter()
            .map(|set| tally.get(&(*set, refusal)).copied().unwrap_or(0))
            .sum();
        assert!(
            by_sets > 0,
            "{ROW}: setup: no text set is refused as {refusal}: {tally:?}"
        );
    }
}
