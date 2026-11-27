//! The public API review before 1.0 (LIB-API-005; ADR 0045) and its fixes
//! (LIB-API-022 to LIB-API-025; slice 0084).
//!
//! - LIB-API-005: every public type is `Send`, `Sync` and `Debug`; `check`
//!   and `preview` take an operation struct as `&op.into()` (D1); the id of
//!   a name, content, value or key is an `ElementId` that `get` does not
//!   find and `is_id_in_use` covers (D2).
//! - LIB-API-022: `LangString` converts from text, with no `lang`; every
//!   constructor's `name` takes `impl Into<LangString>`, and the JSON of an
//!   operation is unchanged.
//! - LIB-API-023: `Display` and `Ord` on `Flag` and `ReferenceEnd`,
//!   `Mode::as_str` and `Display`, `ElementId` and `Href` into `String`, and
//!   `Href` from text exactly as `Href::new`.
//! - LIB-API-024: `MultiLangString` iterates its values; `Mutation::is_empty`;
//!   `ReadOutput::into_parts`.
//! - LIB-API-025: `AddReference::new_href` and `RemoveReference::new_href`,
//!   with `href()`; the JSON is unchanged.
//!
//! "Unchanged" means `==` and the same `revision()`.

use std::collections::BTreeSet;
use std::fmt::Debug;

use sacm::compare::{Changes, Difference};
use sacm::diagnostic::{
    Argument, ArgumentValue, Code, Details, Diagnostic, Severity, SourceLocation, Specification,
};
use sacm::edit::{
    self, AddReference, Batch, BatchError, BatchMutation, CopyElements, CreateActivity,
    CreateArgumentGroup, CreateArgumentPackage, CreateArgumentPackageBinding,
    CreateArgumentPackageInterface, CreateArgumentReasoning, CreateArtifact,
    CreateArtifactAssetRelationship, CreateArtifactGroup, CreateArtifactPackage,
    CreateArtifactPackageBinding, CreateArtifactPackageInterface, CreateArtifactReference,
    CreateAssertedArtifactContext, CreateAssertedArtifactSupport, CreateAssertedContext,
    CreateAssertedEvidence, CreateAssertedInference, CreateAssuranceCasePackage,
    CreateAssuranceCasePackageBinding, CreateAssuranceCasePackageInterface, CreateCategory,
    CreateClaim, CreateDescription, CreateEvent, CreateExpression, CreateImplementationConstraint,
    CreateNote, CreateParticipant, CreateProperty, CreateResource, CreateTaggedValue,
    CreateTechnique, CreateTerm, CreateTerminologyGroup, CreateTerminologyPackage,
    CreateTerminologyPackageBinding, CreateTerminologyPackageInterface,
    CrossPackageReferencePolicy, DanglingReference, DeleteElement, EditError, MoveElement,
    Mutation, Operation, PackageDeletePolicy, Preview, ReferenceDeletePolicy, RemoveReference,
    ReorderElements, SetAssertionDeclaration, SetContent, SetDate, SetEndTime,
    SetExternalReference, SetFlag, SetGid, SetKey, SetLocation, SetName, SetReferences,
    SetStartTime, SetTaggedValue, SetValue, SetVersion,
};
use sacm::model::{
    AbstractClass, AssertionDeclaration, Document, ElementId, ElementKind, Flag, Href, HrefError,
    LangString, MetamodelPackage, MultiLangString, ParseAssertionDeclarationError, Proxy,
    ReferenceEnd, Referrer, Revision, SacmElement, Subclass,
};
use sacm::xmi::{
    self, Limit, Limits, Mode, ReadError, ReadOptions, ReadOutput, WriteAttribute, WriteError,
    WriteOptions,
};

const API_005: &str = "LIB-API-005";
const API_022: &str = "LIB-API-022";
const API_023: &str = "LIB-API-023";
const API_024: &str = "LIB-API-024";
const API_025: &str = "LIB-API-025";

/// The namespace declarations of a root: `sacm` and `xmi`.
const NS: &str = concat!(
    " xmlns:sacm=\"http://www.omg.org/spec/SACM/20220301\"",
    " xmlns:xmi=\"http://www.omg.org/spec/XMI/20131001\"",
);

/// The directory of the golden JSON files.
#[cfg(feature = "serde")]
const GOLDEN: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/tests/data/golden/serde");

// --- Helpers -----------------------------------------------------------------------------

/// A plain `LangString` with the content `text` and no `lang`, built the
/// 0.4.0 way, so that a setup does not depend on what a test checks.
fn ls(text: &str) -> LangString {
    LangString::new(text)
}

/// The `Href` `value`, which `Href::new` must accept.
fn href(row: &str, value: &str) -> Href {
    Href::new(value)
        .unwrap_or_else(|error| panic!("{row}: setup: Href::new({value:?}) refused: {error}"))
}

fn apply(row: &str, what: &str, doc: &mut Document, operation: impl Into<Operation>) -> Mutation {
    edit::apply(doc, operation)
        .unwrap_or_else(|error| panic!("{row}: setup: {what}: refused: {error}"))
}

/// A document with the root ArgumentPackage `p`.
fn package_p(row: &str) -> Document {
    let mut doc = Document::new();
    apply(
        row,
        "the package p",
        &mut doc,
        CreateArgumentPackage::root(ls("P")).with_id("p"),
    );
    doc
}

/// A document with the root ArgumentPackage `p` and the Claims `c1` and
/// `c2` in it.
fn claims(row: &str) -> Document {
    let mut doc = package_p(row);
    for id in ["c1", "c2"] {
        apply(row, id, &mut doc, CreateClaim::new("p", ls(id)).with_id(id));
    }
    doc
}

/// `doc` is unchanged since `before` was cloned from it at `revision`.
fn assert_unchanged(row: &str, what: &str, doc: &Document, before: &Document, revision: Revision) {
    assert_eq!(doc, before, "{row}: {what}: the document changed");
    assert_eq!(
        doc.revision(),
        revision,
        "{row}: {what}: the document's revision changed"
    );
}

/// The refusal of `operation` by `apply`, which must leave `doc` unchanged.
fn refusal(
    row: &str,
    what: &str,
    doc: &mut Document,
    operation: impl Into<Operation>,
) -> EditError {
    let before = doc.clone();
    let revision = doc.revision();
    let error = match edit::apply(doc, operation) {
        Ok(mutation) => panic!("{row}: {what}: accepted, with {mutation:?}"),
        Err(error) => error,
    };
    assert_unchanged(row, what, doc, &before, revision);
    error
}

fn ids(list: &[ElementId]) -> Vec<&str> {
    list.iter().map(ElementId::as_str).collect()
}

// =========================================================================================
// LIB-API-005: the review
// =========================================================================================

/// `T` is `Send`, `Sync` and `Debug`.
fn assert_send_sync_debug<T: Send + Sync + Debug>() {}

#[test]
#[expect(
    clippy::too_many_lines,
    reason = "one list of every public type reads best as one function"
)]
fn lib_api_005_every_public_type_is_send_sync_and_debug() {
    // sacm::model
    assert_send_sync_debug::<Document>();
    assert_send_sync_debug::<SacmElement<'_>>();
    assert_send_sync_debug::<Referrer<'_>>();
    assert_send_sync_debug::<Subclass<'_>>();
    assert_send_sync_debug::<ElementId>();
    assert_send_sync_debug::<Href>();
    assert_send_sync_debug::<HrefError>();
    assert_send_sync_debug::<Proxy>();
    assert_send_sync_debug::<LangString>();
    assert_send_sync_debug::<MultiLangString>();
    assert_send_sync_debug::<ElementKind>();
    assert_send_sync_debug::<AbstractClass>();
    assert_send_sync_debug::<MetamodelPackage>();
    assert_send_sync_debug::<Flag>();
    assert_send_sync_debug::<ReferenceEnd>();
    assert_send_sync_debug::<Revision>();
    assert_send_sync_debug::<AssertionDeclaration>();
    assert_send_sync_debug::<ParseAssertionDeclarationError>();
    // sacm::xmi
    assert_send_sync_debug::<ReadOptions>();
    assert_send_sync_debug::<ReadOutput>();
    assert_send_sync_debug::<ReadError>();
    assert_send_sync_debug::<WriteOptions>();
    assert_send_sync_debug::<WriteError>();
    assert_send_sync_debug::<WriteAttribute>();
    assert_send_sync_debug::<Mode>();
    assert_send_sync_debug::<Limits>();
    assert_send_sync_debug::<Limit>();
    // sacm::diagnostic
    assert_send_sync_debug::<Diagnostic>();
    assert_send_sync_debug::<Code>();
    assert_send_sync_debug::<Severity>();
    assert_send_sync_debug::<Argument>();
    assert_send_sync_debug::<ArgumentValue>();
    assert_send_sync_debug::<Details>();
    assert_send_sync_debug::<SourceLocation>();
    assert_send_sync_debug::<Specification>();
    // sacm::edit: the results, refusals and previews
    assert_send_sync_debug::<Operation>();
    assert_send_sync_debug::<Mutation>();
    assert_send_sync_debug::<BatchMutation>();
    assert_send_sync_debug::<BatchError>();
    assert_send_sync_debug::<EditError>();
    assert_send_sync_debug::<Preview>();
    assert_send_sync_debug::<DanglingReference>();
    assert_send_sync_debug::<Batch<'_>>();
    // sacm::edit: the delete policies
    assert_send_sync_debug::<ReferenceDeletePolicy>();
    assert_send_sync_debug::<PackageDeletePolicy>();
    assert_send_sync_debug::<CrossPackageReferencePolicy>();
    // sacm::edit: every operation struct
    assert_send_sync_debug::<CreateAssuranceCasePackage>();
    assert_send_sync_debug::<CreateAssuranceCasePackageInterface>();
    assert_send_sync_debug::<CreateAssuranceCasePackageBinding>();
    assert_send_sync_debug::<CreateArgumentPackage>();
    assert_send_sync_debug::<CreateArgumentPackageInterface>();
    assert_send_sync_debug::<CreateArgumentPackageBinding>();
    assert_send_sync_debug::<CreateClaim>();
    assert_send_sync_debug::<CreateArgumentReasoning>();
    assert_send_sync_debug::<CreateArtifactReference>();
    assert_send_sync_debug::<CreateArgumentGroup>();
    assert_send_sync_debug::<CreateAssertedInference>();
    assert_send_sync_debug::<CreateAssertedContext>();
    assert_send_sync_debug::<CreateAssertedEvidence>();
    assert_send_sync_debug::<CreateAssertedArtifactSupport>();
    assert_send_sync_debug::<CreateAssertedArtifactContext>();
    assert_send_sync_debug::<CreateDescription>();
    assert_send_sync_debug::<CreateImplementationConstraint>();
    assert_send_sync_debug::<CreateNote>();
    assert_send_sync_debug::<CreateTaggedValue>();
    assert_send_sync_debug::<CreateArtifactPackage>();
    assert_send_sync_debug::<CreateArtifactPackageInterface>();
    assert_send_sync_debug::<CreateArtifactPackageBinding>();
    assert_send_sync_debug::<CreateProperty>();
    assert_send_sync_debug::<CreateTechnique>();
    assert_send_sync_debug::<CreateParticipant>();
    assert_send_sync_debug::<CreateArtifactGroup>();
    assert_send_sync_debug::<CreateArtifact>();
    assert_send_sync_debug::<CreateEvent>();
    assert_send_sync_debug::<CreateResource>();
    assert_send_sync_debug::<CreateActivity>();
    assert_send_sync_debug::<CreateArtifactAssetRelationship>();
    assert_send_sync_debug::<CreateTerminologyPackage>();
    assert_send_sync_debug::<CreateTerminologyPackageInterface>();
    assert_send_sync_debug::<CreateTerminologyPackageBinding>();
    assert_send_sync_debug::<CreateCategory>();
    assert_send_sync_debug::<CreateTerm>();
    assert_send_sync_debug::<CreateExpression>();
    assert_send_sync_debug::<CreateTerminologyGroup>();
    assert_send_sync_debug::<SetName>();
    assert_send_sync_debug::<SetGid>();
    assert_send_sync_debug::<SetFlag>();
    assert_send_sync_debug::<SetAssertionDeclaration>();
    assert_send_sync_debug::<SetContent>();
    assert_send_sync_debug::<SetKey>();
    assert_send_sync_debug::<SetTaggedValue>();
    assert_send_sync_debug::<SetVersion>();
    assert_send_sync_debug::<SetDate>();
    assert_send_sync_debug::<SetStartTime>();
    assert_send_sync_debug::<SetEndTime>();
    assert_send_sync_debug::<SetLocation>();
    assert_send_sync_debug::<SetValue>();
    assert_send_sync_debug::<SetExternalReference>();
    assert_send_sync_debug::<SetReferences>();
    assert_send_sync_debug::<AddReference>();
    assert_send_sync_debug::<RemoveReference>();
    assert_send_sync_debug::<ReorderElements>();
    assert_send_sync_debug::<MoveElement>();
    assert_send_sync_debug::<CopyElements>();
    assert_send_sync_debug::<DeleteElement>();
    // sacm::compare
    assert_send_sync_debug::<Difference>();
    assert_send_sync_debug::<Changes>();

    // The bounds above are checked at compile time. In use: a document read
    // on one thread is shared by two others, and moved to a third.
    let input = format!(
        "<sacm:ArgumentPackage{NS} xmi:id=\"p\"><name content=\"P\"/></sacm:ArgumentPackage>"
    );
    let doc = xmi::read(input.as_bytes(), &ReadOptions::default())
        .unwrap_or_else(|error| panic!("{API_005}: setup: a strict read refuses: {error}"))
        .into_document();
    let found = std::thread::scope(|scope| {
        let first = scope.spawn(|| doc.contains("p"));
        let second = scope.spawn(|| doc.get("p").map(|p| format!("{p:?}")).is_some());
        (first.join(), second.join())
    });
    assert!(
        matches!(found, (Ok(true), Ok(true))),
        "{API_005}: a document shared by two threads: {found:?}"
    );
    let moved = std::thread::spawn(move || doc.element_count()).join();
    assert!(
        matches!(moved, Ok(1)),
        "{API_005}: a document moved to another thread: {moved:?}"
    );
}

#[test]
fn lib_api_005_check_and_preview_take_an_operation_struct_by_into() {
    let doc = package_p(API_005);
    let before = doc.clone();
    let revision = doc.revision();

    // check: the struct as `&op.into()` answers as an `Operation` does.
    assert_eq!(
        edit::check(&doc, &CreateClaim::new("p", "C1").into()),
        Ok(()),
        "{API_005}: check(&doc, &CreateClaim::new(..).into()) does not accept the Claim"
    );
    assert_eq!(
        edit::check(&doc, &CreateClaim::new("p", "C1").into()),
        edit::check(&doc, &Operation::from(CreateClaim::new("p", "C1"))),
        "{API_005}: check answers `&op.into()` otherwise than `&Operation::from(op)`"
    );
    // A refusal is the same refusal both ways.
    let misplaced = CreateClaim::new("nowhere", "C1");
    let refused = edit::check(&doc, &misplaced.clone().into());
    assert!(
        matches!(&refused, Err(EditError::ParentNotFound { parent, .. }) if parent == "nowhere"),
        "{API_005}: check of a Claim in no parent: {refused:?}"
    );
    assert_eq!(
        refused,
        edit::check(&doc, &Operation::from(misplaced)),
        "{API_005}: check refuses `&op.into()` otherwise than `&Operation::from(op)`"
    );

    // preview: the same, and it shows the id apply would give.
    let claim = CreateClaim::new("p", "C1");
    let preview = edit::preview(&doc, &claim.clone().into())
        .unwrap_or_else(|error| panic!("{API_005}: preview of the Claim refused: {error}"));
    assert_eq!(
        preview.mutation().created_id().map(ElementId::as_str),
        Some("claim_1"),
        "{API_005}: the preview's created id"
    );
    let from_operation = edit::preview(&doc, &Operation::from(claim))
        .unwrap_or_else(|error| panic!("{API_005}: preview of the Operation refused: {error}"));
    assert_eq!(
        preview.mutation(),
        from_operation.mutation(),
        "{API_005}: preview of `&op.into()` differs from that of `&Operation::from(op)`"
    );
    assert_unchanged(API_005, "check and preview", &doc, &before, revision);
}

/// The root ArgumentPackage `p` with a name `p_name`, a note `n1` whose
/// content is `n1_content` with the value `n1_en`, and a tagged value `t1`
/// whose key is `t1_key`.
fn part_ids_input() -> String {
    format!(
        "<sacm:ArgumentPackage{NS} xmi:id=\"p\">\
         <name xmi:id=\"p_name\" content=\"P\"/>\
         <note xmi:id=\"n1\"><content xmi:id=\"n1_content\">\
         <value xmi:id=\"n1_en\" lang=\"en\" content=\"Reviewed\"/></content></note>\
         <taggedValue xmi:id=\"t1\"><key xmi:id=\"t1_key\"><value content=\"ASIL\"/></key>\
         <content><value content=\"D\"/></content></taggedValue>\
         </sacm:ArgumentPackage>"
    )
}

#[test]
fn lib_api_005_a_part_id_is_an_element_id_get_does_not_find() {
    let input = part_ids_input();
    let output = xmi::read(input.as_bytes(), &ReadOptions::default())
        .unwrap_or_else(|error| panic!("{API_005}: setup: a strict read refuses: {error}"));
    assert!(
        output.diagnostics().is_empty(),
        "{API_005}: setup: the read reports {:?}",
        output.diagnostics()
    );
    let mut doc = output.into_document();

    let element = |id: &str| {
        doc.get(id)
            .unwrap_or_else(|| panic!("{API_005}: setup: no element {id:?}"))
    };
    // Each part id is an `&ElementId`: the bindings pin the type.
    let name_id: &ElementId = element("p")
        .name_id()
        .unwrap_or_else(|| panic!("{API_005}: p has no name id"));
    let content_id: &ElementId = element("n1")
        .content_id()
        .unwrap_or_else(|| panic!("{API_005}: n1 has no content id"));
    let value_id: &ElementId = element("n1")
        .content_value_ids()
        .first()
        .and_then(Option::as_ref)
        .unwrap_or_else(|| panic!("{API_005}: n1's content value has no id"));
    let key_id: &ElementId = element("t1")
        .key_id()
        .unwrap_or_else(|| panic!("{API_005}: t1 has no key id"));
    let part_ids: Vec<ElementId> = [name_id, content_id, value_id, key_id]
        .into_iter()
        .cloned()
        .collect();
    assert_eq!(
        ids(&part_ids),
        ["p_name", "n1_content", "n1_en", "t1_key"],
        "{API_005}: the part ids read"
    );

    for id in &part_ids {
        assert!(
            doc.get(id).is_none(),
            "{API_005}: get({id:?}) finds an element for a part id"
        );
        assert!(
            !doc.contains(id),
            "{API_005}: contains({id:?}) is true for a part id"
        );
        assert!(
            doc.is_id_in_use(id),
            "{API_005}: is_id_in_use({id:?}) is false for a part id"
        );
    }

    // A create with a part id is refused as `DuplicateId`, and changes
    // nothing.
    for id in &part_ids {
        let what = format!("a Claim with the id {id:?}");
        let error = refusal(
            API_005,
            &what,
            &mut doc,
            CreateClaim::new("p", ls("C")).with_id(id),
        );
        assert!(
            matches!(&error, EditError::DuplicateId { id: given, .. } if given == id),
            "{API_005}: {what}: {error:?}, not DuplicateId"
        );
        assert_eq!(
            error.code(),
            Code::CMD_DUPLICATE_ID,
            "{API_005}: {what}: the code"
        );
        assert_eq!(
            error.code().as_str(),
            "SACM-CMD-004",
            "{API_005}: {what}: the code's text"
        );
    }
}

// =========================================================================================
// LIB-API-022: names take plain text
// =========================================================================================

#[test]
fn lib_api_022_lang_string_from_text_has_no_lang() {
    let from_str = LangString::from("C1");
    assert_eq!(
        from_str,
        LangString::new("C1"),
        "{API_022}: LangString::from(&str) differs from LangString::new"
    );
    assert_eq!(from_str.lang(), None, "{API_022}: from(&str) has a lang");
    assert_eq!(
        from_str.content(),
        Some("C1"),
        "{API_022}: from(&str)'s content"
    );
    assert!(
        !from_str.is_expression_lang_string(),
        "{API_022}: from(&str) is an ExpressionLangString"
    );

    let from_string = LangString::from(String::from("C1"));
    assert_eq!(
        from_string,
        LangString::new("C1"),
        "{API_022}: LangString::from(String) differs from LangString::new"
    );
    assert_eq!(
        from_string.lang(),
        None,
        "{API_022}: from(String) has a lang"
    );

    // `Into` gives the same.
    let into: LangString = "C1".into();
    assert_eq!(into, LangString::new("C1"), "{API_022}: \"C1\".into()");

    // Empty text is an empty content, not none.
    let empty = LangString::from("");
    assert_eq!(
        empty.content(),
        Some(""),
        "{API_022}: LangString::from(\"\") has no content"
    );
    assert_eq!(
        empty,
        LangString::new(""),
        "{API_022}: LangString::from(\"\")"
    );
    assert_ne!(
        empty,
        LangString::default(),
        "{API_022}: LangString::from(\"\") is the default, which has no content"
    );
}

/// The number of constructors with a `name` in this release.
const NAMED_CONSTRUCTORS: usize = 55;

/// The operations of the constructors with a `name`, each built with `name`
/// and labelled with its constructor: every create, root, `new_href` and
/// `root_href` of `crates/sacm/src/edit/`, and `SetName::new`.
#[expect(
    clippy::too_many_lines,
    reason = "one list of every constructor with a name reads best as one function"
)]
fn named_operations<N: Into<LangString> + Clone>(name: &N) -> Vec<(&'static str, Operation)> {
    let n = || name.clone();
    let h = || href(API_022, "other.sacm.xmi#x");
    vec![
        // The Assurance Case package (create.rs).
        (
            "CreateAssuranceCasePackage::root",
            CreateAssuranceCasePackage::root(n()).into(),
        ),
        (
            "CreateAssuranceCasePackage::new",
            CreateAssuranceCasePackage::new("case", n()).into(),
        ),
        (
            "CreateAssuranceCasePackageInterface::root",
            CreateAssuranceCasePackageInterface::root(n(), "case").into(),
        ),
        (
            "CreateAssuranceCasePackageInterface::root_href",
            CreateAssuranceCasePackageInterface::root_href(n(), h()).into(),
        ),
        (
            "CreateAssuranceCasePackageInterface::new_href",
            CreateAssuranceCasePackageInterface::new_href("case", n(), h()).into(),
        ),
        (
            "CreateAssuranceCasePackageInterface::new",
            CreateAssuranceCasePackageInterface::new("case", n(), "case").into(),
        ),
        (
            "CreateAssuranceCasePackageBinding::root",
            CreateAssuranceCasePackageBinding::root(n(), ["i1", "i2"]).into(),
        ),
        (
            "CreateAssuranceCasePackageBinding::new",
            CreateAssuranceCasePackageBinding::new("case", n(), ["i1", "i2"]).into(),
        ),
        // The Argumentation package (create.rs, argumentation.rs).
        (
            "CreateArgumentPackage::root",
            CreateArgumentPackage::root(n()).into(),
        ),
        (
            "CreateArgumentPackage::new",
            CreateArgumentPackage::new("case", n()).into(),
        ),
        (
            "CreateArgumentPackageInterface::root",
            CreateArgumentPackageInterface::root(n(), "p").into(),
        ),
        (
            "CreateArgumentPackageInterface::root_href",
            CreateArgumentPackageInterface::root_href(n(), h()).into(),
        ),
        (
            "CreateArgumentPackageInterface::new_href",
            CreateArgumentPackageInterface::new_href("p", n(), h()).into(),
        ),
        (
            "CreateArgumentPackageInterface::new",
            CreateArgumentPackageInterface::new("p", n(), "p").into(),
        ),
        (
            "CreateArgumentPackageBinding::root",
            CreateArgumentPackageBinding::root(n(), ["i1", "i2"]).into(),
        ),
        (
            "CreateArgumentPackageBinding::new",
            CreateArgumentPackageBinding::new("p", n(), ["i1", "i2"]).into(),
        ),
        ("CreateClaim::new", CreateClaim::new("p", n()).into()),
        (
            "CreateArgumentReasoning::new",
            CreateArgumentReasoning::new("p", n()).into(),
        ),
        (
            "CreateArtifactReference::new",
            CreateArtifactReference::new("p", n()).into(),
        ),
        (
            "CreateArgumentGroup::new",
            CreateArgumentGroup::new("p", n()).into(),
        ),
        (
            "CreateAssertedInference::new",
            CreateAssertedInference::new("p", n(), ["c1"], "c2").into(),
        ),
        (
            "CreateAssertedContext::new",
            CreateAssertedContext::new("p", n(), ["c1"], "c2").into(),
        ),
        (
            "CreateAssertedEvidence::new",
            CreateAssertedEvidence::new("p", n(), ["a1"], "c2").into(),
        ),
        (
            "CreateAssertedArtifactSupport::new",
            CreateAssertedArtifactSupport::new("p", n(), ["a1"], "a2").into(),
        ),
        (
            "CreateAssertedArtifactContext::new",
            CreateAssertedArtifactContext::new("p", n(), ["a1"], "a2").into(),
        ),
        // The Artifact package (artifact.rs).
        (
            "CreateArtifactPackage::root",
            CreateArtifactPackage::root(n()).into(),
        ),
        (
            "CreateArtifactPackage::new",
            CreateArtifactPackage::new("case", n()).into(),
        ),
        (
            "CreateArtifactPackageInterface::root",
            CreateArtifactPackageInterface::root(n(), "a").into(),
        ),
        (
            "CreateArtifactPackageInterface::root_href",
            CreateArtifactPackageInterface::root_href(n(), h()).into(),
        ),
        (
            "CreateArtifactPackageInterface::new_href",
            CreateArtifactPackageInterface::new_href("a", n(), h()).into(),
        ),
        (
            "CreateArtifactPackageInterface::new",
            CreateArtifactPackageInterface::new("a", n(), "a").into(),
        ),
        (
            "CreateArtifactPackageBinding::root",
            CreateArtifactPackageBinding::root(n(), ["i1", "i2"]).into(),
        ),
        (
            "CreateArtifactPackageBinding::new",
            CreateArtifactPackageBinding::new("a", n(), ["i1", "i2"]).into(),
        ),
        ("CreateProperty::new", CreateProperty::new("t1", n()).into()),
        (
            "CreateTechnique::new",
            CreateTechnique::new("a", n()).into(),
        ),
        (
            "CreateParticipant::new",
            CreateParticipant::new("a", n()).into(),
        ),
        (
            "CreateArtifactGroup::new",
            CreateArtifactGroup::new("a", n()).into(),
        ),
        ("CreateArtifact::new", CreateArtifact::new("a", n()).into()),
        ("CreateEvent::new", CreateEvent::new("a", n()).into()),
        ("CreateResource::new", CreateResource::new("a", n()).into()),
        ("CreateActivity::new", CreateActivity::new("a", n()).into()),
        (
            "CreateArtifactAssetRelationship::new",
            CreateArtifactAssetRelationship::new("a", n(), ["x1"], ["x2"]).into(),
        ),
        // The Terminology package (terminology.rs).
        (
            "CreateTerminologyPackage::root",
            CreateTerminologyPackage::root(n()).into(),
        ),
        (
            "CreateTerminologyPackage::new",
            CreateTerminologyPackage::new("case", n()).into(),
        ),
        (
            "CreateTerminologyPackageInterface::root",
            CreateTerminologyPackageInterface::root(n(), "terms").into(),
        ),
        (
            "CreateTerminologyPackageInterface::root_href",
            CreateTerminologyPackageInterface::root_href(n(), h()).into(),
        ),
        (
            "CreateTerminologyPackageInterface::new_href",
            CreateTerminologyPackageInterface::new_href("terms", n(), h()).into(),
        ),
        (
            "CreateTerminologyPackageInterface::new",
            CreateTerminologyPackageInterface::new("terms", n(), "terms").into(),
        ),
        (
            "CreateTerminologyPackageBinding::root",
            CreateTerminologyPackageBinding::root(n(), ["i1", "i2"]).into(),
        ),
        (
            "CreateTerminologyPackageBinding::new",
            CreateTerminologyPackageBinding::new("terms", n(), ["i1", "i2"]).into(),
        ),
        (
            "CreateCategory::new",
            CreateCategory::new("terms", n()).into(),
        ),
        (
            "CreateTerm::new",
            CreateTerm::new("terms", n(), "value").into(),
        ),
        (
            "CreateExpression::new",
            CreateExpression::new("terms", n(), "value").into(),
        ),
        (
            "CreateTerminologyGroup::new",
            CreateTerminologyGroup::new("terms", n()).into(),
        ),
        // The name of any element (set.rs).
        ("SetName::new", SetName::new("c1", n()).into()),
    ]
}

/// The operation names [`named_operations`] covers: every operation with a
/// `name`.
const NAMED_OPERATION_NAMES: [&str; 35] = [
    "CreateAssuranceCasePackage",
    "CreateAssuranceCasePackageInterface",
    "CreateAssuranceCasePackageBinding",
    "CreateArgumentPackage",
    "CreateArgumentPackageInterface",
    "CreateArgumentPackageBinding",
    "CreateClaim",
    "CreateArgumentReasoning",
    "CreateArtifactReference",
    "CreateArgumentGroup",
    "CreateAssertedInference",
    "CreateAssertedContext",
    "CreateAssertedEvidence",
    "CreateAssertedArtifactSupport",
    "CreateAssertedArtifactContext",
    "CreateArtifactPackage",
    "CreateArtifactPackageInterface",
    "CreateArtifactPackageBinding",
    "CreateProperty",
    "CreateTechnique",
    "CreateParticipant",
    "CreateArtifactGroup",
    "CreateArtifact",
    "CreateEvent",
    "CreateResource",
    "CreateActivity",
    "CreateArtifactAssetRelationship",
    "CreateTerminologyPackage",
    "CreateTerminologyPackageInterface",
    "CreateTerminologyPackageBinding",
    "CreateCategory",
    "CreateTerm",
    "CreateExpression",
    "CreateTerminologyGroup",
    "SetName",
];

/// The operations of [`named_operations`] built with `LangString::new("N")`,
/// each beside the same constructor built with `text`.
fn named_pairs<T: Into<LangString> + Clone>(text: &T) -> Vec<(&'static str, Operation, Operation)> {
    let typed = named_operations(&LangString::new("N"));
    let plain = named_operations(text);
    assert_eq!(
        typed.len(),
        NAMED_CONSTRUCTORS,
        "{API_022}: setup: not every constructor with a name is listed"
    );
    let names: BTreeSet<&str> = typed
        .iter()
        .map(|(_, operation)| operation.operation_name())
        .collect();
    assert_eq!(
        names,
        NAMED_OPERATION_NAMES.into_iter().collect::<BTreeSet<_>>(),
        "{API_022}: setup: the constructors listed are not those of every operation with a name"
    );
    typed
        .into_iter()
        .zip(plain)
        .map(|((label, typed), (plain_label, plain))| {
            assert_eq!(label, plain_label, "{API_022}: setup: the lists differ");
            (label, typed, plain)
        })
        .collect()
}

#[test]
fn lib_api_022_every_name_parameter_takes_text() {
    for (label, typed, plain) in named_pairs(&"N") {
        assert_eq!(
            plain, typed,
            "{API_022}: {label} with the name \"N\" differs from it with LangString::new(\"N\")"
        );
    }
    for (label, typed, plain) in named_pairs(&String::from("N")) {
        assert_eq!(
            plain, typed,
            "{API_022}: {label} with the name String::from(\"N\") differs from it with LangString::new(\"N\")"
        );
    }
    // A LangString with a lang is still taken as it is.
    let english = LangString::new("N").with_lang("en");
    assert_ne!(
        Operation::from(CreateClaim::new("p", english.clone())),
        Operation::from(CreateClaim::new("p", "N")),
        "{API_022}: setup: a lang does not tell the names apart"
    );
    assert_eq!(
        CreateClaim::new("p", english.clone()).name(),
        &english,
        "{API_022}: CreateClaim::new with a LangString does not keep it"
    );
}

#[cfg(feature = "serde")]
#[test]
fn lib_api_022_operation_json_is_unchanged() {
    let json = |label: &str, operation: &Operation| {
        serde_json::to_value(operation)
            .unwrap_or_else(|error| panic!("{API_022}: {label} does not serialize: {error}"))
    };
    let name = serde_json::to_value(LangString::new("N"))
        .unwrap_or_else(|error| panic!("{API_022}: setup: the name does not serialize: {error}"));
    for (label, typed, plain) in named_pairs(&"N") {
        let written = json(label, &plain);
        assert_eq!(
            written,
            json(label, &typed),
            "{API_022}: the JSON of {label} with the name \"N\" differs from it with LangString::new(\"N\")"
        );
        assert_eq!(
            written.get("name"),
            Some(&name),
            "{API_022}: the JSON of {label} with the name \"N\" has another \"name\""
        );
    }

    // `operations.json`, unregenerated, reads and writes back byte for byte.
    let path = std::path::Path::new(GOLDEN).join("operations.json");
    let golden = std::fs::read_to_string(&path)
        .unwrap_or_else(|error| panic!("{API_022}: cannot read {}: {error}", path.display()));
    let read: Vec<Operation> = serde_json::from_str(&golden)
        .unwrap_or_else(|error| panic!("{API_022}: operations.json does not read: {error}"));
    let written = serde_json::to_string_pretty(&read)
        .unwrap_or_else(|error| panic!("{API_022}: operations.json does not write: {error}"))
        + "\n";
    assert_eq!(
        written, golden,
        "{API_022}: operations.json does not write back as it is"
    );
}

#[test]
fn lib_api_022_swapped_parent_and_name_is_refused() {
    let mut doc = package_p(API_022);
    // The name "p" is the id of the package, and the parent "C1" names no
    // element: the swap compiles, and apply refuses it.
    let error = refusal(
        API_022,
        "CreateClaim::new(\"C1\", \"p\")",
        &mut doc,
        CreateClaim::new("C1", "p"),
    );
    assert!(
        matches!(&error, EditError::ParentNotFound { parent, .. } if parent == "C1"),
        "{API_022}: the swap is refused with {error:?}, not ParentNotFound for \"C1\""
    );
    assert_eq!(
        error.code(),
        Code::CMD_TARGET_NOT_FOUND,
        "{API_022}: the swap's code"
    );
    assert_eq!(
        error.code().as_str(),
        "SACM-CMD-001",
        "{API_022}: the swap's code text"
    );
}

// =========================================================================================
// LIB-API-023: uniform traits and conversions
// =========================================================================================

#[test]
fn lib_api_023_flag_and_reference_end_display_their_name() {
    for flag in Flag::ALL {
        assert_eq!(
            flag.to_string(),
            flag.name(),
            "{API_023}: {flag:?} displays otherwise than its name()"
        );
        assert_eq!(
            format!("{flag}"),
            flag.name(),
            "{API_023}: {flag:?} formats otherwise than its name()"
        );
    }
    for end in ReferenceEnd::ALL {
        assert_eq!(
            end.to_string(),
            end.name(),
            "{API_023}: {end:?} displays otherwise than its name()"
        );
    }
    for (shown, expected) in [
        (Flag::IsCitation.to_string(), "isCitation"),
        (ReferenceEnd::MetaClaim.to_string(), "metaClaim"),
        (ReferenceEnd::ArtifactSource.to_string(), "source"),
    ] {
        assert_eq!(shown, expected, "{API_023}: Display of a named value");
    }
}

#[test]
fn lib_api_023_mode_as_str_and_display() {
    for (mode, text) in [(Mode::Strict, "strict"), (Mode::Tolerant, "tolerant")] {
        assert_eq!(mode.as_str(), text, "{API_023}: {mode:?}.as_str()");
        assert_eq!(mode.to_string(), text, "{API_023}: {mode:?}.to_string()");
    }
    // `as_str` is `'static`.
    let text: &'static str = Mode::Strict.as_str();
    assert_eq!(text, "strict", "{API_023}: Mode::as_str is not 'static");
}

#[test]
fn lib_api_023_flag_and_reference_end_order_as_declared() {
    assert!(
        Flag::ALL.windows(2).all(|pair| pair[0] < pair[1]),
        "{API_023}: Flag::ALL is not strictly ascending: {:?}",
        Flag::ALL
    );
    assert!(
        ReferenceEnd::ALL.windows(2).all(|pair| pair[0] < pair[1]),
        "{API_023}: ReferenceEnd::ALL is not strictly ascending: {:?}",
        ReferenceEnd::ALL
    );
    // Declaration order, not name order: `isCitation` before `isAbstract`,
    // `source` before `structure`, and the two `source`s apart.
    assert!(
        Flag::IsCitation < Flag::IsAbstract && Flag::IsAbstract < Flag::IsCounter,
        "{API_023}: Flag does not order as declared"
    );
    assert!(
        ReferenceEnd::CitedElement < ReferenceEnd::AbstractForm
            && ReferenceEnd::Source < ReferenceEnd::ArtifactSource
            && ReferenceEnd::ParticipantPackage < ReferenceEnd::Expression,
        "{API_023}: ReferenceEnd does not order as declared"
    );
    // `Ord` agrees with `PartialOrd`: a sorted copy of ALL is ALL.
    let mut ends = ReferenceEnd::ALL.to_vec();
    ends.reverse();
    ends.sort();
    assert_eq!(
        ends,
        ReferenceEnd::ALL,
        "{API_023}: sorting ReferenceEnd does not give ALL"
    );
    let mut flags: BTreeSet<Flag> = BTreeSet::new();
    flags.extend(Flag::ALL.iter().rev().copied());
    assert_eq!(
        flags.into_iter().collect::<Vec<_>>(),
        Flag::ALL,
        "{API_023}: a BTreeSet of Flag does not hold them in ALL's order"
    );
}

#[test]
fn lib_api_023_ids_and_hrefs_convert_into_string() {
    let id = ElementId::from("c1");
    assert_eq!(
        String::from(id.clone()),
        "c1",
        "{API_023}: String::from(ElementId)"
    );
    let into: String = id.into();
    assert_eq!(into, "c1", "{API_023}: ElementId.into()");

    for value in ["other.sacm#c1", "sacm:Claim other.sacm#c1"] {
        let found = href(API_023, value);
        assert_eq!(
            String::from(found.clone()),
            value,
            "{API_023}: String::from(Href) of {value:?}"
        );
        let into: String = found.clone().into();
        assert_eq!(into, found.as_str(), "{API_023}: Href.into() of {value:?}");
    }
}

#[test]
fn lib_api_023_href_parses_as_new_does() {
    for (value, expected) in [
        ("other.sacm#c1", Ok("other.sacm#c1")),
        ("other.sacm", Err(HrefError::NoFragment)),
        ("other.sacm#a#b", Err(HrefError::HashInFragment)),
    ] {
        let new = Href::new(value);
        assert_eq!(
            new.as_ref().map(Href::as_str).map_err(|error| *error),
            expected,
            "{API_023}: setup: Href::new({value:?})"
        );
        assert_eq!(
            value.parse::<Href>(),
            new,
            "{API_023}: {value:?}.parse::<Href>() differs from Href::new"
        );
        assert_eq!(
            Href::try_from(value),
            new,
            "{API_023}: Href::try_from({value:?}) differs from Href::new"
        );
        assert_eq!(
            Href::try_from(value.to_owned()),
            new,
            "{API_023}: Href::try_from(String::from({value:?})) differs from Href::new"
        );
    }
}

// =========================================================================================
// LIB-API-024: collection and result helpers
// =========================================================================================

#[test]
fn lib_api_024_multi_lang_string_iterates_its_values() {
    // A repeat and an order no sort gives.
    let mut ml: MultiLangString = [
        LangString::new("Granskad").with_lang("sv"),
        LangString::new("Reviewed").with_lang("en"),
        LangString::new("Granskad").with_lang("sv"),
        LangString::new("Geprüft"),
    ]
    .into_iter()
    .collect();
    let expected: Vec<LangString> = ml.values().to_vec();

    let by_iter: Vec<LangString> = ml.iter().cloned().collect();
    assert_eq!(
        by_iter, expected,
        "{API_024}: iter() is not values() in order"
    );
    let mut by_ref = Vec::new();
    for value in &ml {
        by_ref.push(value.clone());
    }
    assert_eq!(
        by_ref, expected,
        "{API_024}: `for v in &ml` is not values() in order"
    );
    // The iterators are the slice's.
    let _: std::slice::Iter<'_, LangString> = ml.iter();
    let _: std::slice::Iter<'_, LangString> = <&MultiLangString as IntoIterator>::into_iter(&ml);

    // `&mut` reaches every value, in order.
    let mut seen = Vec::new();
    for value in &mut ml {
        seen.push(value.content().map(str::to_owned));
        value.set_lang(Some("de".to_owned()));
    }
    assert_eq!(
        seen,
        expected
            .iter()
            .map(|value| value.content().map(str::to_owned))
            .collect::<Vec<_>>(),
        "{API_024}: `for v in &mut ml` is not values() in order"
    );
    assert!(
        ml.iter().all(|value| value.lang() == Some("de")),
        "{API_024}: `for v in &mut ml` did not set every lang: {ml:?}"
    );

    let _: std::slice::IterMut<'_, LangString> = ml.iter_mut();
    let _: std::slice::IterMut<'_, LangString> =
        <&mut MultiLangString as IntoIterator>::into_iter(&mut ml);
    ml.iter_mut()
        .for_each(|value| value.set_lang(Some("fr".to_owned())));
    assert!(
        ml.values().iter().all(|value| value.lang() == Some("fr")),
        "{API_024}: iter_mut() did not set every lang: {ml:?}"
    );
    assert_eq!(
        ml.len(),
        4,
        "{API_024}: iterating changed the number of values"
    );
    let contents: Vec<Option<&str>> = ml.iter().map(LangString::content).collect();
    assert_eq!(
        contents,
        [
            Some("Granskad"),
            Some("Reviewed"),
            Some("Granskad"),
            Some("Geprüft")
        ],
        "{API_024}: iter_mut() changed the order or the contents"
    );

    // An empty MultiLangString iterates nothing.
    let mut empty = MultiLangString::default();
    assert_eq!(empty.iter().count(), 0, "{API_024}: an empty iter()");
    assert_eq!(
        empty.iter_mut().count(),
        0,
        "{API_024}: an empty iter_mut()"
    );
}

/// `mutation` is empty exactly when it lists no id.
fn assert_is_empty_agrees(what: &str, mutation: &Mutation, expected: bool) {
    let lists_none = mutation.created_ids().is_empty()
        && mutation.changed_ids().is_empty()
        && mutation.deleted_ids().is_empty();
    assert_eq!(
        lists_none,
        expected,
        "{API_024}: setup: {what}: the mutation lists {:?}, {:?}, {:?}",
        mutation.created_ids(),
        mutation.changed_ids(),
        mutation.deleted_ids()
    );
    assert_eq!(
        mutation.is_empty(),
        expected,
        "{API_024}: {what}: is_empty() of a mutation listing created {:?}, changed {:?}, deleted {:?}",
        mutation.created_ids(),
        mutation.changed_ids(),
        mutation.deleted_ids()
    );
}

#[test]
fn lib_api_024_mutation_is_empty_when_it_lists_no_id() {
    let mut doc = package_p(API_024);
    apply(
        API_024,
        "the package q",
        &mut doc,
        CreateArgumentPackage::root(ls("Q")).with_id("q"),
    );

    // A root reorder changes the document and lists no id.
    let before = doc.clone();
    let reorder = apply(
        API_024,
        "the root reorder",
        &mut doc,
        ReorderElements::roots(["q", "p"]),
    );
    let roots: Vec<&str> = doc.roots().map(|root| root.id().as_str()).collect();
    assert_eq!(
        roots,
        ["q", "p"],
        "{API_024}: setup: the roots were not reordered"
    );
    assert_ne!(doc, before, "{API_024}: setup: the reorder changed nothing");
    assert_is_empty_agrees("ReorderElements::roots", &reorder, true);

    // A set of the value held lists no id.
    let same = apply(
        API_024,
        "the same name",
        &mut doc,
        SetName::new("q", ls("Q")),
    );
    assert_is_empty_agrees("SetName to the name held", &same, true);

    // A changed, a created and a deleted id each make it non-empty.
    let renamed = apply(
        API_024,
        "the new name",
        &mut doc,
        SetName::new("p", ls("P2")),
    );
    assert_eq!(
        ids(renamed.changed_ids()),
        ["p"],
        "{API_024}: setup: the rename"
    );
    assert_is_empty_agrees("SetName to a new name", &renamed, false);

    let created = apply(
        API_024,
        "the Claim",
        &mut doc,
        CreateClaim::new("p", ls("C1")).with_id("c1"),
    );
    assert_eq!(
        ids(created.created_ids()),
        ["c1"],
        "{API_024}: setup: the create"
    );
    assert_is_empty_agrees("CreateClaim", &created, false);

    let deleted = apply(API_024, "the delete", &mut doc, DeleteElement::new("c1"));
    assert_eq!(
        ids(deleted.deleted_ids()),
        ["c1"],
        "{API_024}: setup: the delete"
    );
    assert!(
        deleted.created_ids().is_empty() && deleted.changed_ids().is_empty(),
        "{API_024}: setup: the delete lists more than its deleted id"
    );
    assert_is_empty_agrees("DeleteElement", &deleted, false);
}

#[test]
fn lib_api_024_into_parts_keeps_document_and_diagnostics() {
    let input = format!(
        "<sacm:ArgumentPackage{NS} xmi:id=\" p \"><name content=\"P\"/></sacm:ArgumentPackage>"
    );
    let output = xmi::read(input.as_bytes(), &ReadOptions::default())
        .unwrap_or_else(|error| panic!("{API_024}: setup: a strict read refuses: {error}"));
    let trimmed: Vec<&Diagnostic> = output
        .diagnostics()
        .iter()
        .filter(|found| found.code() == Code::ID_TRIMMED)
        .collect();
    assert!(
        matches!(trimmed.as_slice(), [one] if one.severity() == Severity::Info
            && one.affected().iter().any(|id| id == "p")),
        "{API_024}: setup: the read does not report one SACM-ID-004 Info for p: {:?}",
        output.diagnostics()
    );
    let document = output.document().clone();
    let diagnostics = output.diagnostics().to_vec();
    assert!(!diagnostics.is_empty(), "{API_024}: setup: no diagnostics");

    let (parts_document, parts_diagnostics): (Document, Vec<Diagnostic>) = output.into_parts();
    assert_eq!(
        parts_document, document,
        "{API_024}: into_parts() gives another document"
    );
    assert_eq!(
        parts_diagnostics, diagnostics,
        "{API_024}: into_parts() gives other diagnostics"
    );
    assert!(
        parts_document.contains("p"),
        "{API_024}: into_parts()'s document lacks p"
    );
}

// =========================================================================================
// LIB-API-025: one spelling for the href form
// =========================================================================================

#[test]
fn lib_api_025_add_reference_new_href_holds_the_href() {
    let h = href(API_025, "other.sacm.xmi#m1");
    let add = AddReference::new_href("c1", ReferenceEnd::MetaClaim, h.clone());
    assert_eq!(add.href(), Some(&h), "{API_025}: new_href's href()");
    assert_eq!(add.id(), None, "{API_025}: new_href's id()");
    assert_eq!(add.element(), "c1", "{API_025}: new_href's element()");
    assert_eq!(
        add.end(),
        ReferenceEnd::MetaClaim,
        "{API_025}: new_href's end()"
    );

    let by_id = AddReference::new("c1", ReferenceEnd::MetaClaim, "c2");
    assert_eq!(by_id.href(), None, "{API_025}: the id form's href()");
    assert_eq!(
        by_id.id().map(ElementId::as_str),
        Some("c2"),
        "{API_025}: the id form's id()"
    );

    // It builds the cross-document form: apply appends the href, not an id.
    let mut doc = claims(API_025);
    let mutation = apply(API_025, "new_href", &mut doc, add);
    assert_eq!(
        ids(mutation.changed_ids()),
        ["c1"],
        "{API_025}: the add's changed ids"
    );
    let c1 = doc
        .get("c1")
        .unwrap_or_else(|| panic!("{API_025}: setup: no c1"));
    assert_eq!(
        c1.hrefs(ReferenceEnd::MetaClaim),
        [h],
        "{API_025}: the add did not append the href"
    );
    assert!(
        c1.meta_claims().is_empty(),
        "{API_025}: the add appended an id"
    );
}

#[test]
fn lib_api_025_remove_reference_new_href_holds_the_href() {
    let h = href(API_025, "other.sacm.xmi#m1");
    let remove = RemoveReference::new_href("c1", ReferenceEnd::MetaClaim, h.clone());
    assert_eq!(remove.href(), Some(&h), "{API_025}: new_href's href()");
    assert_eq!(remove.id(), None, "{API_025}: new_href's id()");
    assert_eq!(remove.element(), "c1", "{API_025}: new_href's element()");
    assert_eq!(
        remove.end(),
        ReferenceEnd::MetaClaim,
        "{API_025}: new_href's end()"
    );

    let by_id = RemoveReference::new("c1", ReferenceEnd::MetaClaim, "c2");
    assert_eq!(by_id.href(), None, "{API_025}: the id form's href()");
    assert_eq!(
        by_id.id().map(ElementId::as_str),
        Some("c2"),
        "{API_025}: the id form's id()"
    );

    // It builds the cross-document form: apply removes the href, and the id
    // beside it stays.
    let mut doc = claims(API_025);
    apply(
        API_025,
        "the id",
        &mut doc,
        AddReference::new("c1", ReferenceEnd::MetaClaim, "c2"),
    );
    apply(
        API_025,
        "the href",
        &mut doc,
        AddReference::new_href("c1", ReferenceEnd::MetaClaim, h.clone()),
    );
    let mutation = apply(API_025, "new_href", &mut doc, remove);
    assert_eq!(
        ids(mutation.changed_ids()),
        ["c1"],
        "{API_025}: the remove's changed ids"
    );
    let c1 = doc
        .get("c1")
        .unwrap_or_else(|| panic!("{API_025}: setup: no c1"));
    assert!(
        c1.hrefs(ReferenceEnd::MetaClaim).is_empty(),
        "{API_025}: the remove left the href"
    );
    assert_eq!(
        ids(c1.meta_claims()),
        ["c2"],
        "{API_025}: the remove touched the id"
    );
}

#[cfg(feature = "serde")]
#[test]
fn lib_api_025_href_reference_json_is_unchanged() {
    let path = std::path::Path::new(GOLDEN).join("operations.json");
    let golden = std::fs::read_to_string(&path)
        .unwrap_or_else(|error| panic!("{API_025}: cannot read {}: {error}", path.display()));
    let items: Vec<serde_json::Value> = serde_json::from_str(&golden)
        .unwrap_or_else(|error| panic!("{API_025}: operations.json is not a JSON array: {error}"));
    let remote = href(API_025, "braking.sacm.xmi#braking_api");
    let end = ReferenceEnd::ParticipantPackage;

    let mut seen = Vec::new();
    for item in &items {
        let kind = item.get("type").and_then(serde_json::Value::as_str);
        let is_reference = matches!(kind, Some("AddReference" | "RemoveReference"));
        if !is_reference || item.get("href").is_none() {
            continue;
        }
        let kind = kind.unwrap_or_default();
        let read: Operation = serde_json::from_value(item.clone()).unwrap_or_else(|error| {
            panic!("{API_025}: the {kind} href entry does not read: {error}")
        });
        let expected = match kind {
            "AddReference" => {
                Operation::from(AddReference::new_href("binding", end, remote.clone()))
            }
            _ => Operation::from(RemoveReference::new_href("binding", end, remote.clone())),
        };
        assert_eq!(
            read, expected,
            "{API_025}: the {kind} href entry of operations.json does not read as new_href"
        );
        let written = serde_json::to_value(&expected)
            .unwrap_or_else(|error| panic!("{API_025}: new_href's {kind} does not write: {error}"));
        assert_eq!(
            &written, item,
            "{API_025}: new_href's {kind} does not write the entry of operations.json"
        );
        seen.push(kind.to_owned());
    }
    assert_eq!(
        seen,
        ["AddReference", "RemoveReference"],
        "{API_025}: setup: operations.json does not hold one href add and one href remove"
    );
}
