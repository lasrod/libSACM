//! Tests for the `serde` feature (LIB-API-006; ADR 0030, shape rules R1-R7).
//!
//! - Golden files under `tests/data/golden/serde/` fix the JSON shape, which
//!   is a stability promise: serializing the samples built here with
//!   `serde_json::to_string_pretty` gives each file's bytes (plus a final line
//!   feed), and reading the file gives values `==` to the samples. The files
//!   are written by hand from R1-R7: an enum with data is tagged with
//!   `"type"` first (R1), an `EditError` writes `"code"` right after it (R7),
//!   keys are the `snake_case` accessor names in declaration order (R2), and
//!   every key is written, `null` for none and `[]` for empty (R3).
//! - Every operation struct and constructor form, a `Mutation`, every
//!   `EditError` variant (both forms of `ChildNotFound` and `InvalidOrder`), a
//!   full and a minimal `Diagnostic`, and `LangString` and `MultiLangString`
//!   values are sampled. The samples hold every `Operation` and `EditError`
//!   variant of this release; the exhaustive `match` that fails to compile
//!   when a variant has no sample must live in the crate, because both enums
//!   are `#[non_exhaustive]`.
//! - Names and codes round-trip as strings (R5); unknown types, keys, names
//!   and codes, missing required keys, a mismatched `EditError` code and a
//!   location of 0 are refused (R3-R7). Each refusal is shown against a
//!   valid twin, so that it fails for the reason named and not for a typo.
//! - A deserialized operation is `==` to its constructed twin, and `check`
//!   and `apply` answer it exactly as the twin (ADR 0030 "Deserializing
//!   proves nothing about origin").
//!
//! Not checked here:
//! - That `Document` implements neither trait: a `compile_fail` doctest in the
//!   crate.
//! - That `sacm` has no serde dependency without the feature, and the wasm32,
//!   `cargo deny` and MSRV builds with it: CI and the gate.

#![cfg(feature = "serde")]

use std::collections::BTreeSet;
use std::fmt::Debug;
use std::path::Path;

use sacm::diagnostic::{Code, Diagnostic, Severity, SourceLocation};
use sacm::edit::{
    self, AddReference, CreateArgumentGroup, CreateArgumentPackage, CreateArgumentReasoning,
    CreateArtifactReference, CreateAssertedArtifactContext, CreateAssertedArtifactSupport,
    CreateAssertedContext, CreateAssertedEvidence, CreateAssertedInference,
    CreateAssuranceCasePackage, CreateClaim, CreateDescription, CreateImplementationConstraint,
    CreateNote, CreateTaggedValue, EditError, Mutation, Operation, RemoveReference,
    ReorderElements, SetAssertionDeclaration, SetContent, SetFlag, SetGid, SetKey, SetName,
    SetReferences,
};
use sacm::model::{
    AssertionDeclaration, Document, ElementId, ElementKind, Flag, LangString, MultiLangString,
    ReferenceEnd,
};
use sacm::xmi::{self, ReadOptions};
use serde::Serialize;
use serde::de::DeserializeOwned;

const API_006: &str = "LIB-API-006";

/// The directory of the golden JSON files.
const GOLDEN: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/tests/data/golden/serde");

/// Every operation name of this release, in the declaration order of
/// `Operation`.
const OPERATION_NAMES: [&str; 25] = [
    "CreateAssuranceCasePackage",
    "CreateArgumentPackage",
    "CreateClaim",
    "CreateArgumentReasoning",
    "CreateAssertedInference",
    "CreateAssertedContext",
    "CreateArtifactReference",
    "CreateArgumentGroup",
    "CreateAssertedEvidence",
    "CreateAssertedArtifactSupport",
    "CreateAssertedArtifactContext",
    "CreateDescription",
    "CreateImplementationConstraint",
    "CreateNote",
    "CreateTaggedValue",
    "SetName",
    "SetGid",
    "SetFlag",
    "SetAssertionDeclaration",
    "SetContent",
    "SetKey",
    "SetReferences",
    "AddReference",
    "RemoveReference",
    "ReorderElements",
];

/// Every `EditError` variant of this release, with its catalog code, in
/// declaration order.
const EDIT_ERRORS: [(&str, &str); 14] = [
    ("ParentNotFound", "SACM-CMD-001"),
    ("InvalidParent", "SACM-CMD-005"),
    ("InvalidId", "SACM-ID-002"),
    ("DuplicateId", "SACM-CMD-004"),
    ("TooFewReferences", "SACM-MULT-001"),
    ("TooManyReferences", "SACM-MULT-001"),
    ("InvalidReference", "SACM-ID-002"),
    ("ReferenceNotFound", "SACM-CMD-001"),
    ("WrongReferenceType", "SACM-REF-002"),
    ("ElementNotFound", "SACM-CMD-001"),
    ("FeatureNotApplicable", "SACM-CMD-008"),
    ("ReferenceNotHeld", "SACM-CMD-001"),
    ("ChildNotFound", "SACM-CMD-001"),
    ("InvalidOrder", "SACM-CMD-009"),
];

/// Every catalog `Code` constant of this release. The catalog cross-check
/// keeps it in step with `docs/sacm/diagnostics.md`.
const CODES: [Code; 22] = [
    Code::XML_MALFORMED,
    Code::XML_DOCTYPE_REJECTED,
    Code::XMI_INVALID_ROOT,
    Code::XMI_MISSING_ID,
    Code::XMI_XSI_TYPE,
    Code::XMI_VERSION_IGNORED,
    Code::MULTIPLICITY_VIOLATION,
    Code::ENUM_INVALID_LITERAL,
    Code::CMD_TARGET_NOT_FOUND,
    Code::CMD_DUPLICATE_ID,
    Code::CMD_INVALID_PARENT,
    Code::CMD_FEATURE_NOT_APPLICABLE,
    Code::CMD_INVALID_ORDER,
    Code::ID_DUPLICATE,
    Code::ID_INVALID,
    Code::GID_DUPLICATE,
    Code::ID_TRIMMED,
    Code::PACKAGE_CONTENT_INVALID,
    Code::REF_DANGLING,
    Code::REF_WRONG_TYPE,
    Code::CITATION_INVALID,
    Code::ABSTRACT_FORM_INVALID,
];

/// The three flags, with their names.
const FLAGS: [(Flag, &str); 3] = [
    (Flag::IsCitation, "isCitation"),
    (Flag::IsAbstract, "isAbstract"),
    (Flag::IsCounter, "isCounter"),
];

/// The nine reference ends, with their names.
const ENDS: [(ReferenceEnd, &str); 9] = [
    (ReferenceEnd::CitedElement, "citedElement"),
    (ReferenceEnd::AbstractForm, "abstractForm"),
    (ReferenceEnd::MetaClaim, "metaClaim"),
    (ReferenceEnd::Reasoning, "reasoning"),
    (ReferenceEnd::Source, "source"),
    (ReferenceEnd::Target, "target"),
    (ReferenceEnd::Structure, "structure"),
    (
        ReferenceEnd::ReferencedArtifactElement,
        "referencedArtifactElement",
    ),
    (ReferenceEnd::GroupMember, "argumentElement"),
];

/// The three severities, with their names.
const SEVERITIES: [(Severity, &str); 3] = [
    (Severity::Info, "info"),
    (Severity::Warning, "warning"),
    (Severity::Error, "error"),
];

/// The document the edit errors are refused on: the AssuranceCasePackage
/// `case`, its ArgumentPackage `p`, and in it the Claim `c1` and a Claim with
/// the id `a b` (not an `NCName`, kept as read).
const ERROR_START: &str = r#"<sacm:AssuranceCasePackage xmlns:sacm="http://www.omg.org/spec/SACM/20220301" xmlns:xmi="http://www.omg.org/spec/XMI/20131001" xmi:id="case">
  <name content="Case"/>
  <argumentPackage xmi:id="p">
    <name content="P"/>
    <argumentElement xmi:type="sacm:Claim" xmi:id="c1"><name content="C1"/></argumentElement>
    <argumentElement xmi:type="sacm:Claim" xmi:id="a b"><name content="Spaced"/></argumentElement>
  </argumentPackage>
</sacm:AssuranceCasePackage>
"#;

/// A valid `CreateClaim`, compact: the example of ADR 0030.
const CLAIM_JSON: &str = r#"{"type":"CreateClaim","parent":"arguments","id":null,"name":{"lang":"en","content":"The system is safe"}}"#;

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

fn id(value: &str) -> ElementId {
    ElementId::from(value)
}

fn en(content: &str) -> LangString {
    LangString::new(content).with_lang("en")
}

/// Two values in two languages.
fn two_values() -> MultiLangString {
    [en("Reviewed"), LangString::new("Granskad").with_lang("sv")]
        .into_iter()
        .collect()
}

/// `value` as compact JSON.
fn to_json<T: Serialize + Debug>(value: &T) -> String {
    match serde_json::to_string(value) {
        Ok(text) => text,
        Err(error) => panic!("{API_006}: {value:?} does not serialize: {error}"),
    }
}

/// `text` read as a `T`, which it must be.
fn from_json<T: DeserializeOwned>(text: &str) -> T {
    match serde_json::from_str(text) {
        Ok(value) => value,
        Err(error) => panic!("{API_006}: {text} does not read: {error}"),
    }
}

/// The golden file `name`, as text.
fn golden(name: &str) -> String {
    let path = Path::new(GOLDEN).join(name);
    match std::fs::read_to_string(&path) {
        Ok(text) => text,
        Err(error) => panic!("{API_006}: cannot read {}: {error}", path.display()),
    }
}

/// Serializing `values` pretty gives the bytes of the golden file `name`,
/// and reading the file gives values `==` to `values`.
fn assert_golden<T>(name: &str, values: &T)
where
    T: Serialize + DeserializeOwned + PartialEq + Debug,
{
    let expected = golden(name);
    let written = match serde_json::to_string_pretty(values) {
        Ok(text) => text + "\n",
        Err(error) => panic!("{API_006}: the samples of {name} do not serialize: {error}"),
    };
    assert_eq!(
        written, expected,
        "{API_006}: serializing the samples does not give the bytes of {name}"
    );
    let read: T = match serde_json::from_str(&expected) {
        Ok(read) => read,
        Err(error) => panic!("{API_006}: {name} does not read back: {error}"),
    };
    assert_eq!(
        &read, values,
        "{API_006}: {name} reads back to values other than the samples"
    );
}

/// `valid` reads as a `T`, and `valid` with its first `from` replaced by
/// `to` is refused as data (not as malformed JSON): the refusal is caused by
/// the change, which `what` names.
fn assert_refused<T: DeserializeOwned + Debug>(what: &str, valid: &str, from: &str, to: &str) {
    let invalid = valid.replacen(from, to, 1);
    assert_ne!(
        invalid, valid,
        "{API_006}: setup: {what}: {from:?} is not in {valid}"
    );
    if let Err(error) = serde_json::from_str::<T>(valid) {
        panic!("{API_006}: setup: {what}: the valid twin {valid} does not read: {error}");
    }
    match serde_json::from_str::<T>(&invalid) {
        Ok(value) => panic!("{API_006}: {what}: {invalid} was read as {value:?}"),
        Err(error) => assert!(
            error.is_data(),
            "{API_006}: {what}: {invalid} is refused as malformed JSON, not as data: {error}"
        ),
    }
}

/// `value` serializes as the JSON string `name` and reads back to `value`.
fn assert_name_round_trips<T>(value: T, name: &str)
where
    T: Serialize + DeserializeOwned + PartialEq + Debug + Copy,
{
    let written = to_json(&value);
    assert_eq!(
        written,
        format!("\"{name}\""),
        "{API_006}: {value:?} does not serialize as its name"
    );
    let read: T = from_json(&written);
    assert_eq!(read, value, "{API_006}: {name:?} does not read back");
}

/// Applies `operation`, which must be refused, to a clone of `doc`, and
/// returns the refusal.
fn refusal(doc: &Document, operation: impl Into<Operation>) -> EditError {
    let operation = operation.into();
    match edit::apply(&mut doc.clone(), operation.clone()) {
        Ok(mutation) => panic!("{API_006}: setup: {operation:?} was accepted: {mutation:?}"),
        Err(error) => error,
    }
}

/// Applies `operation`, which must be accepted, to `doc`.
fn applied(doc: &mut Document, operation: impl Into<Operation>) -> Mutation {
    let operation = operation.into();
    match edit::apply(doc, operation.clone()) {
        Ok(mutation) => mutation,
        Err(error) => panic!("{API_006}: setup: {operation:?} was refused: {error}"),
    }
}

/// The variant name of `error`, from its `Debug`.
fn variant_of(error: &EditError) -> String {
    let shown = format!("{error:?}");
    shown
        .split([' ', '{', '('])
        .next()
        .unwrap_or_default()
        .to_owned()
}

/// `SourceLocation` at `line` and `column`, which must not be 0.
fn location(line: u32, column: u32) -> SourceLocation {
    let Some(location) = SourceLocation::new(line, column) else {
        panic!("{API_006}: setup: no location at {line}:{column}");
    };
    location
}

fn error_start() -> Document {
    match xmi::read(ERROR_START.as_bytes(), &ReadOptions::default()) {
        Ok(output) => output.into_document(),
        Err(error) => panic!("{API_006}: setup: ERROR_START does not read: {error}"),
    }
}

// ---------------------------------------------------------------------------
// The samples
// ---------------------------------------------------------------------------

/// Every operation struct in every constructor form: root and parent, with
/// and without id, with and without each builder, set and remove, set and
/// clear, children and roots. In the order of `operations.json`.
fn operation_samples() -> Vec<Operation> {
    let mut samples = package_and_claim_samples();
    samples.extend(argumentation_samples());
    samples.extend(utility_samples());
    samples.extend(edit_samples());
    samples
}

/// The package and Claim creates of [`operation_samples`].
fn package_and_claim_samples() -> Vec<Operation> {
    vec![
        CreateAssuranceCasePackage::root(en("Safety case")).into(),
        CreateAssuranceCasePackage::root(en("Safety case"))
            .with_id("case")
            .into(),
        CreateAssuranceCasePackage::new("case", en("Subsystem")).into(),
        CreateAssuranceCasePackage::new("case", en("Subsystem"))
            .with_id("sub")
            .into(),
        CreateArgumentPackage::root(en("Arguments")).into(),
        CreateArgumentPackage::root(en("Arguments"))
            .with_id("arguments")
            .into(),
        CreateArgumentPackage::new("case", en("Arguments")).into(),
        CreateArgumentPackage::new("case", en("Arguments"))
            .with_id("arguments")
            .into(),
        CreateClaim::new("arguments", en("The system is safe")).into(),
        CreateClaim::new("arguments", LangString::default())
            .with_id("c1")
            .into(),
    ]
}

/// The creates of [`operation_samples`] that reference other elements.
fn argumentation_samples() -> Vec<Operation> {
    let sources = ["c2", "c3"];
    vec![
        CreateArgumentReasoning::new("arguments", en("Why")).into(),
        CreateArgumentReasoning::new("arguments", en("Why"))
            .with_id("r1")
            .with_structure("arguments")
            .into(),
        CreateAssertedInference::new("arguments", en("Inference"), sources, "c1").into(),
        CreateAssertedInference::new("arguments", en("Inference"), sources, "c1")
            .with_id("i1")
            .with_reasoning("r1")
            .with_is_counter(true)
            .into(),
        CreateAssertedContext::new("arguments", en("Context"), ["x1"], "c1").into(),
        CreateAssertedContext::new("arguments", en("Context"), ["x1"], "c1")
            .with_id("ctx1")
            .with_reasoning("r1")
            .with_is_counter(true)
            .into(),
        CreateAssertedEvidence::new("arguments", en("Evidence"), ["e1"], "c1").into(),
        CreateAssertedEvidence::new("arguments", en("Evidence"), ["e1"], "c1")
            .with_id("ev1")
            .with_reasoning("r1")
            .with_is_counter(true)
            .into(),
        CreateAssertedArtifactSupport::new("arguments", en("Support"), ["e1"], "e2").into(),
        CreateAssertedArtifactSupport::new("arguments", en("Support"), ["e1"], "e2")
            .with_id("s1")
            .with_reasoning("r1")
            .with_is_counter(true)
            .into(),
        CreateAssertedArtifactContext::new("arguments", en("Artifact context"), ["e1"], "e2")
            .into(),
        CreateAssertedArtifactContext::new("arguments", en("Artifact context"), ["e1"], "e2")
            .with_id("ac1")
            .with_reasoning("r1")
            .with_is_counter(true)
            .into(),
        CreateArtifactReference::new("arguments", en("Report")).into(),
        CreateArtifactReference::new("arguments", en("Report"))
            .with_id("e1")
            .with_referenced_artifact_elements(["artifact_1", "artifact_2"])
            .into(),
        CreateArgumentGroup::new("arguments", en("Group")).into(),
        CreateArgumentGroup::new("arguments", en("Group"))
            .with_id("g1")
            .with_group_members(["c1", "c2"])
            .into(),
    ]
}

/// The `UtilityElement` creates of [`operation_samples`].
fn utility_samples() -> Vec<Operation> {
    vec![
        CreateDescription::new("c1", en("Why it holds")).into(),
        CreateDescription::new("c1", two_values())
            .with_id("d1")
            .into(),
        CreateImplementationConstraint::new("c1", en("Only in mode A")).into(),
        CreateImplementationConstraint::new("c1", en("Only in mode A"))
            .with_id("ic1")
            .into(),
        CreateNote::new("c1", MultiLangString::default()).into(),
        CreateNote::new("c1", en("Reviewed")).with_id("n1").into(),
        CreateTaggedValue::new("c1", en("ASIL"), LangString::new("D")).into(),
        CreateTaggedValue::new("c1", en("ASIL"), LangString::new("D"))
            .with_id("t1")
            .into(),
    ]
}

/// The operations of [`operation_samples`] that change an element.
fn edit_samples() -> Vec<Operation> {
    vec![
        SetName::new("c1", en("The system is safe enough")).into(),
        SetGid::new("c1", "G-1").into(),
        SetGid::remove("c1").into(),
        SetFlag::new("c1", Flag::IsCitation, true).into(),
        SetFlag::new("i1", Flag::IsCounter, false).into(),
        SetAssertionDeclaration::new("c1", AssertionDeclaration::NeedsSupport).into(),
        SetContent::new("d1", two_values()).into(),
        SetContent::remove("d1").into(),
        SetKey::new("t1", en("SIL")).into(),
        SetReferences::new("i1", ReferenceEnd::Source, ["c2", "c3"]).into(),
        SetReferences::clear("c1", ReferenceEnd::MetaClaim).into(),
        AddReference::new("g1", ReferenceEnd::GroupMember, "c3").into(),
        RemoveReference::new("i1", ReferenceEnd::Source, "c3").into(),
        ReorderElements::new("arguments", ["c2", "c1"]).into(),
        ReorderElements::roots(["case", "arguments"]).into(),
    ]
}

/// Three mutations, as `edit::apply` returns them: a create with a generated
/// id, a flag set, and a create with a caller id. In the order of
/// `mutations.json`.
fn mutation_samples() -> Vec<Mutation> {
    let mut doc = Document::new();
    vec![
        applied(&mut doc, CreateArgumentPackage::root(en("Arguments"))),
        applied(
            &mut doc,
            SetFlag::new("argument_package_1", Flag::IsAbstract, true),
        ),
        applied(
            &mut doc,
            CreateClaim::new("argument_package_1", LangString::new("C1")).with_id("c1"),
        ),
    ]
}

/// Every `EditError` variant, as `edit::apply` refuses on [`ERROR_START`],
/// with both forms of `ChildNotFound` and `InvalidOrder`, and a second
/// `WrongReferenceType` and `FeatureNotApplicable`. In the order of
/// `edit-errors.json`.
fn edit_error_samples() -> Vec<EditError> {
    let doc = error_start();
    let name = || LangString::new("N");
    vec![
        refusal(&doc, CreateClaim::new("nowhere", name())),
        refusal(&doc, CreateClaim::new("case", name())),
        refusal(&doc, CreateClaim::new("p", name()).with_id("1bad")),
        refusal(&doc, CreateClaim::new("p", name()).with_id("c1")),
        refusal(
            &doc,
            CreateAssertedInference::new("p", name(), Vec::<ElementId>::new(), "c1"),
        ),
        refusal(
            &doc,
            SetReferences::new("c1", ReferenceEnd::CitedElement, ["c1", "c1"]),
        ),
        refusal(
            &doc,
            CreateAssertedInference::new("p", name(), ["a b"], "c1"),
        ),
        refusal(
            &doc,
            CreateAssertedInference::new("p", name(), ["c1"], "nowhere"),
        ),
        refusal(
            &doc,
            CreateArgumentReasoning::new("p", name()).with_structure("c1"),
        ),
        refusal(&doc, CreateAssertedInference::new("p", name(), ["p"], "c1")),
        refusal(&doc, SetName::new("nowhere", name())),
        refusal(&doc, SetFlag::new("c1", Flag::IsCounter, true)),
        refusal(&doc, SetReferences::new("c1", ReferenceEnd::Source, ["c1"])),
        refusal(
            &doc,
            RemoveReference::new("c1", ReferenceEnd::MetaClaim, "m1"),
        ),
        refusal(&doc, ReorderElements::new("p", ["case"])),
        refusal(&doc, ReorderElements::roots(["c1"])),
        refusal(&doc, ReorderElements::new("p", Vec::<ElementId>::new())),
        refusal(&doc, ReorderElements::new("p", ["c1", "c1"])),
    ]
}

/// A full diagnostic (clause, two affected ids, location) and a minimal one
/// (none of them). In the order of `diagnostics.json`.
fn diagnostic_samples() -> Vec<Diagnostic> {
    vec![
        Diagnostic::new(
            Code::REF_DANGLING,
            Severity::Error,
            "SACM23-VAL-003",
            "Claim \"c1\" has the metaClaim \"x\", which names no element of the document",
        )
        .with_clause("11.10")
        .with_affected([id("c1"), id("x")])
        .with_location(location(4, 20)),
        Diagnostic::new(
            Code::XMI_MISSING_ID,
            Severity::Info,
            "SACM23-XMI-020",
            "a Claim has no xmi:id; it is read as claim_1",
        ),
    ]
}

/// The values of `values.json`: a `LangString` absent, with empty content,
/// lang-only, full, and with non-ASCII and quoted content; a
/// `MultiLangString` of two values, empty, and of one absent value; a
/// `SourceLocation`; and an `ElementId`.
type Values = (
    LangString,
    LangString,
    LangString,
    LangString,
    LangString,
    MultiLangString,
    MultiLangString,
    MultiLangString,
    SourceLocation,
    ElementId,
);

fn value_samples() -> Values {
    (
        LangString::default(),
        LangString::new(""),
        LangString::default().with_lang("en"),
        en("The system is safe"),
        LangString::new("S\u{e4}kerhet \u{2713} <&> \"citat\"").with_lang("sv-SE"),
        two_values(),
        MultiLangString::default(),
        MultiLangString::from(LangString::default()),
        location(4, 20),
        id("c1"),
    )
}

// ---------------------------------------------------------------------------
// The traits
// ---------------------------------------------------------------------------

fn assert_serde<T: Serialize + DeserializeOwned>() {}

#[test]
fn lib_api_006_every_listed_type_implements_serialize_and_deserialize() {
    assert_serde::<Operation>();
    assert_serde::<CreateAssuranceCasePackage>();
    assert_serde::<CreateArgumentPackage>();
    assert_serde::<CreateClaim>();
    assert_serde::<CreateArgumentReasoning>();
    assert_serde::<CreateAssertedInference>();
    assert_serde::<CreateAssertedContext>();
    assert_serde::<CreateArtifactReference>();
    assert_serde::<CreateArgumentGroup>();
    assert_serde::<CreateAssertedEvidence>();
    assert_serde::<CreateAssertedArtifactSupport>();
    assert_serde::<CreateAssertedArtifactContext>();
    assert_serde::<CreateDescription>();
    assert_serde::<CreateImplementationConstraint>();
    assert_serde::<CreateNote>();
    assert_serde::<CreateTaggedValue>();
    assert_serde::<SetName>();
    assert_serde::<SetGid>();
    assert_serde::<SetFlag>();
    assert_serde::<SetAssertionDeclaration>();
    assert_serde::<SetContent>();
    assert_serde::<SetKey>();
    assert_serde::<SetReferences>();
    assert_serde::<AddReference>();
    assert_serde::<RemoveReference>();
    assert_serde::<ReorderElements>();
    assert_serde::<Mutation>();
    assert_serde::<EditError>();
    assert_serde::<ElementId>();
    assert_serde::<ElementKind>();
    assert_serde::<LangString>();
    assert_serde::<MultiLangString>();
    assert_serde::<AssertionDeclaration>();
    assert_serde::<Flag>();
    assert_serde::<ReferenceEnd>();
    assert_serde::<Diagnostic>();
    assert_serde::<Code>();
    assert_serde::<Severity>();
    assert_serde::<SourceLocation>();
}

// ---------------------------------------------------------------------------
// The golden files
// ---------------------------------------------------------------------------

#[test]
fn lib_api_006_operations_serialize_to_the_golden_json_and_read_back_equal() {
    let samples = operation_samples();
    let names: BTreeSet<&str> = samples.iter().map(Operation::operation_name).collect();
    assert_eq!(
        names,
        OPERATION_NAMES.into_iter().collect::<BTreeSet<_>>(),
        "{API_006}: setup: the samples do not hold every Operation variant"
    );
    assert_golden("operations.json", &samples);
}

#[test]
fn lib_api_006_every_operation_is_tagged_first_with_its_operation_name() {
    for operation in operation_samples() {
        let name = operation.operation_name();
        let written = to_json(&operation);
        assert!(
            written.starts_with(&format!("{{\"type\":\"{name}\",")),
            "{API_006}: {operation:?} is not written with \"type\":\"{name}\" first: {written}"
        );
    }
}

/// A bare operation struct serializes as its `Operation` without the
/// `"type"` key (R1 tags the enum), and reads back equal.
fn assert_bare<T>(value: &T)
where
    T: Serialize + DeserializeOwned + Into<Operation> + Clone + PartialEq + Debug,
{
    let operation: Operation = value.clone().into();
    let tagged = to_json(&operation);
    let tag = format!("\"type\":\"{}\",", operation.operation_name());
    let bare = to_json(value);
    assert_eq!(
        bare,
        tagged.replacen(&tag, "", 1),
        "{API_006}: the bare {value:?} is not its Operation without \"type\""
    );
    let read: T = from_json(&bare);
    assert_eq!(
        &read, value,
        "{API_006}: the bare {bare} does not read back"
    );
}

#[test]
fn lib_api_006_an_operation_struct_serializes_as_its_operation_without_the_tag() {
    let name = || en("N");
    assert_bare(&CreateAssuranceCasePackage::root(name()));
    assert_bare(&CreateArgumentPackage::new("case", name()).with_id("a"));
    assert_bare(&CreateClaim::new("a", name()));
    assert_bare(&CreateArgumentReasoning::new("a", name()).with_structure("a"));
    assert_bare(&CreateAssertedInference::new("a", name(), ["c1"], "c2"));
    assert_bare(&CreateAssertedContext::new("a", name(), ["c1"], "c2").with_is_counter(true));
    assert_bare(
        &CreateArtifactReference::new("a", name()).with_referenced_artifact_elements(["x"]),
    );
    assert_bare(&CreateArgumentGroup::new("a", name()).with_group_members(["c1"]));
    assert_bare(&CreateAssertedEvidence::new("a", name(), ["e1"], "c1").with_reasoning("r1"));
    assert_bare(&CreateAssertedArtifactSupport::new(
        "a",
        name(),
        ["e1"],
        "e2",
    ));
    assert_bare(&CreateAssertedArtifactContext::new("a", name(), ["e1"], "e2").with_id("x"));
    assert_bare(&CreateDescription::new("c1", two_values()));
    assert_bare(&CreateImplementationConstraint::new("c1", name()));
    assert_bare(&CreateNote::new("c1", name()));
    assert_bare(&CreateTaggedValue::new("c1", name(), name()));
    assert_bare(&SetName::new("c1", name()));
    assert_bare(&SetGid::remove("c1"));
    assert_bare(&SetFlag::new("c1", Flag::IsAbstract, true));
    assert_bare(&SetAssertionDeclaration::new(
        "c1",
        AssertionDeclaration::AsCited,
    ));
    assert_bare(&SetContent::remove("d1"));
    assert_bare(&SetKey::new("t1", name()));
    assert_bare(&SetReferences::clear("c1", ReferenceEnd::AbstractForm));
    assert_bare(&AddReference::new("c1", ReferenceEnd::MetaClaim, "m1"));
    assert_bare(&RemoveReference::new("c1", ReferenceEnd::MetaClaim, "m1"));
    assert_bare(&ReorderElements::roots(["a"]));
}

#[test]
fn lib_api_006_mutations_serialize_to_the_golden_json_and_read_back_equal() {
    assert_golden("mutations.json", &mutation_samples());
}

#[test]
fn lib_api_006_edit_errors_serialize_to_the_golden_json_and_read_back_equal() {
    let samples = edit_error_samples();
    let variants: BTreeSet<String> = samples.iter().map(variant_of).collect();
    let expected: BTreeSet<String> = EDIT_ERRORS
        .iter()
        .map(|(variant, _)| (*variant).to_owned())
        .collect();
    assert_eq!(
        variants, expected,
        "{API_006}: setup: the samples do not hold every EditError variant"
    );
    assert_golden("edit-errors.json", &samples);
}

#[test]
fn lib_api_006_every_edit_error_is_tagged_with_its_variant_then_its_code() {
    for error in edit_error_samples() {
        let variant = variant_of(&error);
        let code = error.code().as_str();
        let catalog = EDIT_ERRORS
            .iter()
            .find(|(name, _)| *name == variant)
            .map(|(_, code)| *code);
        assert_eq!(
            catalog,
            Some(code),
            "{API_006}: setup: {variant} has the code {code}"
        );
        let written = to_json(&error);
        assert!(
            written.starts_with(&format!("{{\"type\":\"{variant}\",\"code\":\"{code}\"")),
            "{API_006}: {error:?} is not written with \"type\" and then \"code\" first: {written}"
        );
    }
}

#[test]
fn lib_api_006_diagnostics_serialize_to_the_golden_json_and_read_back_equal() {
    assert_golden("diagnostics.json", &diagnostic_samples());
}

#[test]
fn lib_api_006_values_serialize_to_the_golden_json_and_read_back_equal() {
    assert_golden("values.json", &value_samples());
}

// ---------------------------------------------------------------------------
// Names and codes (R5, R6)
// ---------------------------------------------------------------------------

#[test]
fn lib_api_006_every_element_kind_round_trips_as_its_name() {
    for &kind in ElementKind::ALL {
        assert_name_round_trips(kind, kind.name());
    }
}

#[test]
fn lib_api_006_every_flag_round_trips_as_its_name() {
    for (flag, name) in FLAGS {
        assert_eq!(flag.name(), name, "{API_006}: setup: the name of {flag:?}");
        assert_name_round_trips(flag, name);
    }
}

#[test]
fn lib_api_006_every_reference_end_round_trips_as_its_name() {
    for (end, name) in ENDS {
        assert_eq!(end.name(), name, "{API_006}: setup: the name of {end:?}");
        assert_name_round_trips(end, name);
    }
}

#[test]
fn lib_api_006_every_assertion_declaration_round_trips_as_its_name() {
    for &declaration in AssertionDeclaration::ALL {
        assert_name_round_trips(declaration, declaration.as_str());
    }
    assert_name_round_trips(AssertionDeclaration::NeedsSupport, "needsSupport");
}

#[test]
fn lib_api_006_every_severity_round_trips_as_its_name() {
    for (severity, name) in SEVERITIES {
        assert_eq!(
            severity.as_str(),
            name,
            "{API_006}: setup: the name of {severity:?}"
        );
        assert_name_round_trips(severity, name);
    }
}

/// The codes of the catalog rows of `docs/sacm/diagnostics.md`, as (code,
/// live): a row is live when its meaning names a `` `Code::NAME` `` (the rule
/// of `diagnostic_catalog.rs`, LIB-DIAG-002), and reserved otherwise.
fn catalog_codes() -> Vec<(String, bool)> {
    let path = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../docs/sacm/diagnostics.md");
    let text = match std::fs::read_to_string(&path) {
        Ok(text) => text,
        Err(error) => panic!("{API_006}: cannot read {}: {error}", path.display()),
    };
    text.lines()
        .filter_map(|line| {
            let line = line.trim();
            let rest = line.strip_prefix("| SACM-")?;
            let (code, meaning) = rest.split_once('|')?;
            Some((format!("SACM-{}", code.trim()), meaning.contains("`Code::")))
        })
        .collect()
}

#[test]
fn lib_api_006_every_catalog_code_round_trips_as_its_code_and_a_reserved_one_is_refused() {
    let catalog = catalog_codes();
    let live: BTreeSet<&str> = catalog
        .iter()
        .filter(|(_, live)| *live)
        .map(|(code, _)| code.as_str())
        .collect();
    let constants: BTreeSet<&str> = CODES.into_iter().map(Code::as_str).collect();
    assert_eq!(
        constants, live,
        "{API_006}: setup: the codes listed here are not the live codes of the catalog"
    );
    for code in CODES {
        assert_name_round_trips(code, code.as_str());
    }
    let reserved: Vec<&str> = catalog
        .iter()
        .filter(|(_, live)| !*live)
        .map(|(code, _)| code.as_str())
        .collect();
    assert!(
        !reserved.is_empty(),
        "{API_006}: setup: the catalog has no reserved code"
    );
    for code in reserved {
        assert_refused::<Code>(
            &format!("the reserved code {code}"),
            "\"SACM-CMD-001\"",
            "SACM-CMD-001",
            code,
        );
    }
}

#[test]
fn lib_api_006_an_element_id_is_a_string_read_unchecked() {
    for value in ["c1", "a b", "1bad", "", "x:y"] {
        let element = id(value);
        let written = to_json(&element);
        assert_eq!(
            written,
            to_json(&value),
            "{API_006}: the id {value:?} is not written as a string"
        );
        let read: ElementId = from_json(&written);
        assert_eq!(
            read, element,
            "{API_006}: the id {value:?} does not read back"
        );
    }
}

// ---------------------------------------------------------------------------
// Refusals (R3-R7)
// ---------------------------------------------------------------------------

#[test]
fn lib_api_006_an_unknown_type_is_refused() {
    assert_refused::<Operation>(
        "an unknown operation",
        CLAIM_JSON,
        "CreateClaim",
        "CreateWidget",
    );
    assert_refused::<Operation>(
        "an operation name in another case",
        CLAIM_JSON,
        "CreateClaim",
        "createClaim",
    );
    assert_refused::<Operation>(
        "an operation with no type",
        CLAIM_JSON,
        r#""type":"CreateClaim","#,
        "",
    );
    let error = r#"{"type":"ParentNotFound","code":"SACM-CMD-001","parent":"x"}"#;
    assert_refused::<EditError>("an unknown edit error", error, "ParentNotFound", "Widget");
    assert_refused::<EditError>(
        "an edit error with no type",
        error,
        r#""type":"ParentNotFound","#,
        "",
    );
}

#[test]
fn lib_api_006_an_unknown_key_is_refused() {
    let operations = [
        ("an unknown key in a CreateClaim", CLAIM_JSON),
        (
            "an unknown key in an asserted relationship",
            r#"{"type":"CreateAssertedInference","parent":"p","id":null,"name":{"lang":null,"content":"I"},"sources":["c1"],"target":"c2","reasoning":null,"is_counter":false}"#,
        ),
        (
            "an unknown key in a CreateTaggedValue",
            r#"{"type":"CreateTaggedValue","parent":"c1","id":null,"key":[],"content":[]}"#,
        ),
        (
            "an unknown key in a SetFlag",
            r#"{"type":"SetFlag","element":"c1","flag":"isCitation","value":true}"#,
        ),
        (
            "an unknown key in a ReorderElements",
            r#"{"type":"ReorderElements","parent":null,"order":["p"]}"#,
        ),
    ];
    for (what, valid) in operations {
        assert_refused::<Operation>(what, valid, "{", r#"{"nmae":null,"#);
    }
    // A misspelt optional key must not fall back to its default.
    assert_refused::<Operation>("a misspelt id", CLAIM_JSON, r#""id":null"#, r#""idd":"c1""#);
    assert_refused::<Operation>(
        "a misspelt is_counter",
        r#"{"type":"CreateAssertedContext","parent":"p","name":{},"sources":["c1"],"target":"c2"}"#,
        r#""target":"c2""#,
        r#""target":"c2","isCounter":true"#,
    );
    assert_refused::<Operation>(
        "an unknown key in a name",
        CLAIM_JSON,
        r#""lang":"en""#,
        r#""language":"en""#,
    );
    let mutation = r#"{"operation":{"type":"SetGid","element":"c1","gid":null},"created_ids":[],"changed_ids":["c1"],"deleted_ids":[]}"#;
    assert_refused::<Mutation>(
        "an unknown key in a Mutation",
        mutation,
        "{",
        r#"{"undone":true,"#,
    );
    let error = r#"{"type":"InvalidOrder","code":"SACM-CMD-009","repeated":null}"#;
    assert_refused::<EditError>(
        "an unknown key in an EditError",
        error,
        r#""repeated":null"#,
        r#""repeated":null,"id":"c1""#,
    );
    let diagnostic = r#"{"code":"SACM-ID-003","severity":"error","requirement_id":"SACM23-VAL-002","clause":"8.2","affected":["c1","c2"],"location":null,"message":"m"}"#;
    assert_refused::<Diagnostic>(
        "an unknown key in a Diagnostic",
        diagnostic,
        r#""message":"m""#,
        r#""message":"m","operation":"SetGid""#,
    );
    assert_refused::<Diagnostic>(
        "the CLI's key requirement in a Diagnostic",
        diagnostic,
        "requirement_id",
        "requirement",
    );
    assert_refused::<LangString>(
        "an unknown key in a LangString",
        r#"{"lang":"en","content":"c"}"#,
        r#""lang""#,
        r#""language""#,
    );
    assert_refused::<SourceLocation>(
        "an unknown key in a SourceLocation",
        r#"{"line":1,"column":2}"#,
        r#""column""#,
        r#""col""#,
    );
}

/// Operations without a required key: (what, a valid operation, the key and
/// its value, which the test removes).
const MISSING_KEYS: [(&str, &str, &str); 17] = [
    (
        "a CreateClaim without parent",
        CLAIM_JSON,
        r#""parent":"arguments","#,
    ),
    (
        "a CreateClaim without name",
        CLAIM_JSON,
        r#","name":{"lang":"en","content":"The system is safe"}"#,
    ),
    (
        "a relationship without sources",
        r#"{"type":"CreateAssertedEvidence","parent":"p","name":{},"sources":["e1"],"target":"c1"}"#,
        r#""sources":["e1"],"#,
    ),
    (
        "a relationship without target",
        r#"{"type":"CreateAssertedEvidence","parent":"p","name":{},"sources":["e1"],"target":"c1"}"#,
        r#","target":"c1""#,
    ),
    (
        "a CreateNote without content",
        r#"{"type":"CreateNote","parent":"c1","content":[]}"#,
        r#","content":[]"#,
    ),
    (
        "a CreateTaggedValue without key",
        r#"{"type":"CreateTaggedValue","parent":"c1","key":[],"content":[]}"#,
        r#""key":[],"#,
    ),
    (
        "a SetName without element",
        r#"{"type":"SetName","element":"c1","name":{}}"#,
        r#""element":"c1","#,
    ),
    (
        "a SetName without name",
        r#"{"type":"SetName","element":"c1","name":{}}"#,
        r#","name":{}"#,
    ),
    (
        "a SetGid without gid",
        r#"{"type":"SetGid","element":"c1","gid":null}"#,
        r#","gid":null"#,
    ),
    (
        "a SetFlag without value",
        r#"{"type":"SetFlag","element":"c1","flag":"isAbstract","value":true}"#,
        r#","value":true"#,
    ),
    (
        "a SetAssertionDeclaration without declaration",
        r#"{"type":"SetAssertionDeclaration","element":"c1","declaration":"assumed"}"#,
        r#","declaration":"assumed""#,
    ),
    (
        "a SetContent without content",
        r#"{"type":"SetContent","element":"d1","content":null}"#,
        r#","content":null"#,
    ),
    (
        "a SetKey without key",
        r#"{"type":"SetKey","element":"t1","key":[]}"#,
        r#","key":[]"#,
    ),
    (
        "a SetReferences without ids",
        r#"{"type":"SetReferences","element":"c1","end":"metaClaim","ids":[]}"#,
        r#","ids":[]"#,
    ),
    (
        "an AddReference without end",
        r#"{"type":"AddReference","element":"c1","end":"metaClaim","id":"m1"}"#,
        r#""end":"metaClaim","#,
    ),
    (
        "a ReorderElements without parent",
        r#"{"type":"ReorderElements","parent":null,"order":["p"]}"#,
        r#""parent":null,"#,
    ),
    (
        "a ReorderElements without order",
        r#"{"type":"ReorderElements","parent":null,"order":["p"]}"#,
        r#","order":["p"]"#,
    ),
];

#[test]
fn lib_api_006_a_package_create_without_parent_is_refused() {
    let root = r#"{"type":"CreateArgumentPackage","parent":null,"id":null,"name":{"lang":null,"content":"A"}}"#;
    assert_refused::<Operation>(
        "a package create without parent",
        root,
        r#""parent":null,"#,
        "",
    );
    let case = r#"{"type":"CreateAssuranceCasePackage","parent":null,"name":{}}"#;
    assert_refused::<Operation>(
        "an AssuranceCasePackage create without parent",
        case,
        r#""parent":null,"#,
        "",
    );
}

#[test]
fn lib_api_006_an_operation_without_a_required_key_is_refused() {
    for (what, valid, key) in MISSING_KEYS {
        assert_refused::<Operation>(what, valid, key, "");
    }
}

#[test]
fn lib_api_006_a_record_without_a_required_key_is_refused() {
    let mutation = r#"{"operation":{"type":"SetGid","element":"c1","gid":null},"created_ids":[],"changed_ids":["c1"],"deleted_ids":[]}"#;
    for key in [
        r#"{"operation":{"type":"SetGid","element":"c1","gid":null},"#,
        r#""created_ids":[],"#,
        r#""changed_ids":["c1"],"#,
        r#","deleted_ids":[]"#,
    ] {
        let replacement = if key.starts_with('{') { "{" } else { "" };
        assert_refused::<Mutation>(
            &format!("a Mutation without {key}"),
            mutation,
            key,
            replacement,
        );
    }
    let error = r#"{"type":"ChildNotFound","code":"SACM-CMD-001","parent":null,"child":"c1"}"#;
    assert_refused::<EditError>(
        "a ChildNotFound without parent",
        error,
        r#""parent":null,"#,
        "",
    );
    assert_refused::<EditError>(
        "a ChildNotFound without child",
        error,
        r#","child":"c1""#,
        "",
    );
    assert_refused::<EditError>(
        "an EditError without code",
        error,
        r#""code":"SACM-CMD-001","#,
        "",
    );
    let diagnostic = r#"{"code":"SACM-ID-003","severity":"error","requirement_id":"SACM23-VAL-002","message":"m"}"#;
    for key in [
        r#""code":"SACM-ID-003","#,
        r#""severity":"error","#,
        r#""requirement_id":"SACM23-VAL-002","#,
        r#","message":"m""#,
    ] {
        assert_refused::<Diagnostic>(&format!("a Diagnostic without {key}"), diagnostic, key, "");
    }
    for key in [r#""line":1,"#, r#","column":2"#] {
        assert_refused::<SourceLocation>(
            &format!("a SourceLocation without {key}"),
            r#"{"line":1,"column":2}"#,
            key,
            "",
        );
    }
}

#[test]
fn lib_api_006_an_unknown_code_severity_or_kind_name_is_refused() {
    // Code.
    let diagnostic = r#"{"code":"SACM-ID-003","severity":"warning","requirement_id":"SACM23-VAL-002","message":"m"}"#;
    assert_refused::<Diagnostic>("an unknown code", diagnostic, "SACM-ID-003", "SACM-XYZ-999");
    assert_refused::<Diagnostic>(
        "a code in lower case",
        diagnostic,
        "SACM-ID-003",
        "sacm-id-003",
    );
    assert_refused::<Diagnostic>(
        "a code with spaces",
        diagnostic,
        "SACM-ID-003",
        " SACM-ID-003",
    );
    assert_refused::<Code>(
        "an unknown code alone",
        "\"SACM-ID-003\"",
        "SACM-ID-003",
        "SACM-ID-999",
    );
    // Severity.
    assert_refused::<Diagnostic>("an unknown severity", diagnostic, "warning", "fatal");
    assert_refused::<Severity>("a severity by its Rust name", "\"error\"", "error", "Error");
    // ElementKind.
    assert_refused::<ElementKind>("an unknown kind", "\"Claim\"", "Claim", "Widget");
    assert_refused::<ElementKind>("a kind in lower case", "\"Claim\"", "Claim", "claim");
    assert_refused::<ElementKind>(
        "an abstract class as a kind",
        "\"Claim\"",
        "Claim",
        "ArgumentAsset",
    );
    let invalid_parent = r#"{"type":"InvalidParent","code":"SACM-CMD-005","parent":"case","parent_kind":"AssuranceCasePackage","kind":"Claim"}"#;
    assert_refused::<EditError>(
        "an unknown parent_kind",
        invalid_parent,
        "AssuranceCasePackage",
        "Widget",
    );
}

#[test]
fn lib_api_006_an_unknown_flag_end_feature_class_or_declaration_name_is_refused() {
    // Flag.
    let flag = r#"{"type":"SetFlag","element":"c1","flag":"isCounter","value":true}"#;
    assert_refused::<Operation>("an unknown flag", flag, "isCounter", "isWidget");
    assert_refused::<Operation>("a flag by its Rust name", flag, "isCounter", "IsCounter");
    // ReferenceEnd.
    let end = r#"{"type":"SetReferences","element":"g1","end":"argumentElement","ids":["c1"]}"#;
    assert_refused::<Operation>("an unknown end", end, "argumentElement", "widget");
    assert_refused::<Operation>(
        "an end by its Rust name",
        end,
        "argumentElement",
        "GroupMember",
    );
    assert_refused::<Operation>(
        "the spelling of the clause text",
        end,
        "argumentElement",
        "argumentationElement",
    );
    let too_few = r#"{"type":"TooFewReferences","code":"SACM-MULT-001","end":"source","given":0,"required":1}"#;
    assert_refused::<EditError>(
        "an unknown end in an EditError",
        too_few,
        "\"source\"",
        "\"sources\"",
    );
    let not_held =
        r#"{"type":"ReferenceNotHeld","code":"SACM-CMD-001","end":"metaClaim","id":"m1"}"#;
    assert_refused::<EditError>(
        "an unknown end in a ReferenceNotHeld",
        not_held,
        "metaClaim",
        "metaclaim",
    );
    // Feature.
    let feature = r#"{"type":"FeatureNotApplicable","code":"SACM-CMD-008","element":"c1","kind":"Claim","feature":"isCounter"}"#;
    assert_refused::<EditError>("an unknown feature", feature, "\"isCounter\"", "\"widget\"");
    assert_refused::<EditError>(
        "an unknown kind in an EditError",
        feature,
        "\"Claim\"",
        "\"Widget\"",
    );
    // Expected class.
    let wrong = r#"{"type":"WrongReferenceType","code":"SACM-REF-002","end":"source","id":"p","kind":"ArgumentPackage","expected":"ArgumentAsset"}"#;
    assert_refused::<EditError>(
        "an unknown expected class",
        wrong,
        "\"ArgumentAsset\"",
        "\"Widget\"",
    );
    // AssertionDeclaration.
    let declaration =
        r#"{"type":"SetAssertionDeclaration","element":"c1","declaration":"needsSupport"}"#;
    assert_refused::<Operation>(
        "an unknown declaration",
        declaration,
        "needsSupport",
        "proven",
    );
    assert_refused::<Operation>(
        "a declaration by its Rust name",
        declaration,
        "needsSupport",
        "NeedsSupport",
    );
}

#[test]
fn lib_api_006_an_edit_error_whose_code_does_not_match_its_variant_is_refused() {
    for error in edit_error_samples() {
        let valid = to_json(&error);
        let code = error.code().as_str();
        // Another code of the catalog, and one that is no code at all.
        let other = if code == "SACM-CMD-004" {
            "SACM-CMD-001"
        } else {
            "SACM-CMD-004"
        };
        for wrong in [other, "SACM-ZZZ-001"] {
            assert_refused::<EditError>(
                &format!("{} with the code {wrong}", variant_of(&error)),
                &valid,
                &format!("\"code\":\"{code}\""),
                &format!("\"code\":\"{wrong}\""),
            );
        }
    }
}

#[test]
fn lib_api_006_a_location_line_or_column_of_0_is_refused() {
    let at = r#"{"line":4,"column":20}"#;
    assert_refused::<SourceLocation>("line 0", at, "4", "0");
    assert_refused::<SourceLocation>("column 0", at, "20", "0");
    assert_refused::<SourceLocation>("a negative line", at, "4", "-4");
    assert_refused::<SourceLocation>("a line beyond u32", at, "4", "4294967296");
    let diagnostic = r#"{"code":"SACM-ID-003","severity":"error","requirement_id":"SACM23-VAL-002","location":{"line":4,"column":20},"message":"m"}"#;
    assert_refused::<Diagnostic>(
        "a diagnostic at line 0",
        diagnostic,
        "\"line\":4",
        "\"line\":0",
    );
    assert_refused::<Diagnostic>(
        "a diagnostic at column 0",
        diagnostic,
        "\"column\":20",
        "\"column\":0",
    );
}

// ---------------------------------------------------------------------------
// Defaults and twins
// ---------------------------------------------------------------------------

#[test]
fn lib_api_006_an_omitted_optional_key_reads_as_the_constructor_default() {
    let name = || LangString::new("N");
    let cases: Vec<(&str, Operation)> = vec![
        (
            r#"{"type":"CreateAssuranceCasePackage","parent":null,"name":{"content":"N"}}"#,
            CreateAssuranceCasePackage::root(name()).into(),
        ),
        (
            r#"{"type":"CreateArgumentPackage","parent":"case","name":{"content":"N"}}"#,
            CreateArgumentPackage::new("case", name()).into(),
        ),
        (
            r#"{"type":"CreateClaim","parent":"p","name":{"content":"N"}}"#,
            CreateClaim::new("p", name()).into(),
        ),
        (
            r#"{"type":"CreateClaim","parent":"p","id":null,"name":{"lang":null,"content":"N"}}"#,
            CreateClaim::new("p", name()).into(),
        ),
        (
            r#"{"type":"CreateArgumentReasoning","parent":"p","name":{"content":"N"}}"#,
            CreateArgumentReasoning::new("p", name()).into(),
        ),
        (
            r#"{"type":"CreateAssertedInference","parent":"p","name":{"content":"N"},"sources":["c2"],"target":"c1"}"#,
            CreateAssertedInference::new("p", name(), ["c2"], "c1").into(),
        ),
        (
            r#"{"type":"CreateAssertedContext","parent":"p","name":{"content":"N"},"sources":["c2"],"target":"c1"}"#,
            CreateAssertedContext::new("p", name(), ["c2"], "c1").into(),
        ),
        (
            r#"{"type":"CreateAssertedEvidence","parent":"p","name":{"content":"N"},"sources":["e1"],"target":"c1"}"#,
            CreateAssertedEvidence::new("p", name(), ["e1"], "c1").into(),
        ),
        (
            r#"{"type":"CreateAssertedArtifactSupport","parent":"p","name":{"content":"N"},"sources":["e1"],"target":"e2"}"#,
            CreateAssertedArtifactSupport::new("p", name(), ["e1"], "e2").into(),
        ),
        (
            r#"{"type":"CreateAssertedArtifactContext","parent":"p","name":{"content":"N"},"sources":["e1"],"target":"e2"}"#,
            CreateAssertedArtifactContext::new("p", name(), ["e1"], "e2").into(),
        ),
        (
            r#"{"type":"CreateArtifactReference","parent":"p","name":{"content":"N"}}"#,
            CreateArtifactReference::new("p", name()).into(),
        ),
        (
            r#"{"type":"CreateArgumentGroup","parent":"p","name":{"content":"N"}}"#,
            CreateArgumentGroup::new("p", name()).into(),
        ),
        (
            r#"{"type":"CreateDescription","parent":"c1","content":[{"content":"N"}]}"#,
            CreateDescription::new("c1", name()).into(),
        ),
        (
            r#"{"type":"CreateImplementationConstraint","parent":"c1","content":[{"content":"N"}]}"#,
            CreateImplementationConstraint::new("c1", name()).into(),
        ),
        (
            r#"{"type":"CreateNote","parent":"c1","content":[]}"#,
            CreateNote::new("c1", MultiLangString::default()).into(),
        ),
        (
            r#"{"type":"CreateTaggedValue","parent":"c1","key":[{"content":"N"}],"content":[{}]}"#,
            CreateTaggedValue::new("c1", name(), LangString::default()).into(),
        ),
    ];
    for (text, expected) in cases {
        let read: Operation = from_json(text);
        assert_eq!(
            read, expected,
            "{API_006}: {text} does not read as its constructor default"
        );
    }
    let diagnostic: Diagnostic = from_json(
        r#"{"code":"SACM-XMI-004","severity":"info","requirement_id":"SACM23-XMI-020","message":"a Claim has no xmi:id; it is read as claim_1"}"#,
    );
    assert_eq!(
        diagnostic,
        diagnostic_samples()[1],
        "{API_006}: a Diagnostic without clause, affected and location is not the minimal one"
    );
}

#[test]
fn lib_api_006_an_empty_lang_string_object_differs_from_empty_content() {
    let empty: LangString = from_json("{}");
    assert_eq!(
        empty,
        LangString::default(),
        "{API_006}: {{}} is not LangString::default()"
    );
    let empty_content: LangString = from_json(r#"{"content":""}"#);
    assert_eq!(
        empty_content,
        LangString::new(""),
        "{API_006}: {{\"content\":\"\"}} is not LangString::new(\"\")"
    );
    assert_ne!(
        empty, empty_content,
        "{API_006}: {{}} and {{\"content\":\"\"}} read as the same LangString (ADR 0012)"
    );
    let lang_only: LangString = from_json(r#"{"lang":"en"}"#);
    assert_eq!(
        lang_only,
        LangString::default().with_lang("en"),
        "{API_006}: {{\"lang\":\"en\"}} is not a lang-only LangString"
    );
    let nulls: LangString = from_json(r#"{"lang":null,"content":null}"#);
    assert_eq!(
        nulls, empty,
        "{API_006}: explicit nulls are not the absent LangString"
    );
}

#[test]
fn lib_api_006_a_deserialized_operation_equals_its_constructed_twin() {
    let read: Operation = from_json(CLAIM_JSON);
    assert_eq!(
        read,
        Operation::from(CreateClaim::new("arguments", en("The system is safe"))),
        "{API_006}: the example of ADR 0030 does not read as its twin"
    );
    let read: Operation = from_json(
        r#"{"type":"SetReferences","element":"g1","end":"argumentElement","ids":["c1","c1"]}"#,
    );
    assert_eq!(
        read,
        Operation::from(SetReferences::new(
            "g1",
            ReferenceEnd::GroupMember,
            ["c1", "c1"]
        )),
        "{API_006}: a SetReferences with a repeated id does not read as its twin"
    );
    // Through a Mutation too: its operation reads as the one applied.
    for mutation in mutation_samples() {
        let read: Mutation = from_json(&to_json(&mutation));
        assert_eq!(
            read.operation(),
            mutation.operation(),
            "{API_006}: a Mutation's operation does not read back"
        );
    }
}

/// `text` read as an `Operation` is `==` to `twin`, and `check` and `apply`
/// answer it on `doc` exactly as they answer `twin`, which `expected`
/// accepts.
fn assert_checked_as_twin(
    doc: &Document,
    text: &str,
    twin: impl Into<Operation>,
    expected: impl Fn(&EditError) -> bool,
) {
    let twin = twin.into();
    let read: Operation = from_json(text);
    assert_eq!(read, twin, "{API_006}: {text} does not read as its twin");
    let answer = edit::apply(&mut doc.clone(), twin.clone()).map(drop);
    assert!(
        matches!(&answer, Err(error) if expected(error)),
        "{API_006}: setup: the twin {twin:?} gives {answer:?}"
    );
    assert_eq!(
        edit::check(doc, &read),
        answer,
        "{API_006}: check answers {text} otherwise than its twin"
    );
    assert_eq!(
        edit::apply(&mut doc.clone(), read).map(drop),
        answer,
        "{API_006}: apply answers {text} otherwise than its twin"
    );
}

#[test]
fn lib_api_006_check_and_apply_refuse_an_invalid_deserialized_operation_as_its_twin() {
    let doc = error_start();
    assert_checked_as_twin(
        &doc,
        r#"{"type":"CreateClaim","parent":"p","id":"a b","name":{"lang":"en","content":"C"}}"#,
        CreateClaim::new("p", en("C")).with_id("a b"),
        |error| matches!(error, EditError::InvalidId { id, .. } if id.as_str() == "a b"),
    );
    assert_checked_as_twin(
        &doc,
        r#"{"type":"CreateNote","parent":"c1","id":"1bad","content":[]}"#,
        CreateNote::new("c1", MultiLangString::default()).with_id("1bad"),
        |error| matches!(error, EditError::InvalidId { id, .. } if id.as_str() == "1bad"),
    );
    assert_checked_as_twin(
        &doc,
        r#"{"type":"CreateAssertedInference","parent":"p","name":{},"sources":[],"target":"c1"}"#,
        CreateAssertedInference::new("p", LangString::default(), Vec::<ElementId>::new(), "c1"),
        |error| matches!(error, EditError::TooFewReferences { end: "source", .. }),
    );
    assert_checked_as_twin(
        &doc,
        r#"{"type":"AddReference","element":"c1","end":"metaClaim","id":"a b"}"#,
        AddReference::new("c1", ReferenceEnd::MetaClaim, "a b"),
        |error| {
            matches!(
                error,
                EditError::InvalidReference {
                    end: "metaClaim",
                    ..
                }
            )
        },
    );
    assert_checked_as_twin(
        &doc,
        r#"{"type":"ReorderElements","parent":"p","order":["c1","c1"]}"#,
        ReorderElements::new("p", ["c1", "c1"]),
        |error| {
            matches!(
                error,
                EditError::InvalidOrder {
                    repeated: Some(_),
                    ..
                }
            )
        },
    );
}
