//! Tests for the `serde` feature (LIB-API-006; ADR 0030, shape rules R1-R7),
//! and for the values slice 0048 adds to it (ADR 0031, ADR 0032):
//! `DeleteElement` and its policies (LIB-EDIT-005, LIB-EDIT-006), the
//! `EditError` variants `Referenced`, `NotEmpty`, `CrossPackageReference`
//! (LIB-EDIT-005, LIB-EDIT-006) and `PreviewExpired` (LIB-EDIT-007),
//! `DanglingReference` and the write-only `Preview` (LIB-EDIT-004), and
//! `BatchMutation` and `BatchError` (LIB-EDIT-014).
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
//! - A `Revision` is process-dependent (ADR 0031), so no golden file holds a
//!   real one: the `PreviewExpired` sample of `edit-errors.json` is read from
//!   JSON with the revisions 1 and 2, a real stale preview's refusal is
//!   round-tripped on its own, and `previews.json` holds each `"revision"`
//!   as 0, which the test writes in place of the real number after checking
//!   it is one.
//!
//! Not checked here:
//! - That `Document` implements neither trait, and that `Preview` does not
//!   implement `Deserialize`: `compile_fail` doctests in the crate.
//! - That `sacm` has no serde dependency without the feature, and the wasm32,
//!   `cargo deny` and MSRV builds with it: CI and the gate.

#![cfg(feature = "serde")]

use std::collections::BTreeSet;
use std::fmt::Debug;
use std::path::Path;

use sacm::diagnostic::{Code, Diagnostic, Severity, SourceLocation};
use sacm::edit::{
    self, AddReference, BatchError, BatchMutation, CreateArgumentGroup, CreateArgumentPackage,
    CreateArgumentReasoning, CreateArtifactGroup, CreateArtifactPackage, CreateArtifactReference,
    CreateAssertedArtifactContext, CreateAssertedArtifactSupport, CreateAssertedContext,
    CreateAssertedEvidence, CreateAssertedInference, CreateAssuranceCasePackage, CreateClaim,
    CreateDescription, CreateImplementationConstraint, CreateNote, CreateParticipant,
    CreateProperty, CreateTaggedValue, CreateTechnique, CrossPackageReferencePolicy,
    DanglingReference, DeleteElement, EditError, Mutation, Operation, PackageDeletePolicy, Preview,
    ReferenceDeletePolicy, RemoveReference, ReorderElements, SetAssertionDeclaration, SetContent,
    SetFlag, SetGid, SetKey, SetName, SetReferences,
};
// The operations of slice 0055 (ADR 0034 amendment 1).
use sacm::edit::{
    CreateActivity, CreateArtifact, CreateEvent, CreateResource, SetDate, SetEndTime, SetLocation,
    SetStartTime, SetVersion,
};
// The operations of slice 0056 (ADR 0036, ADR 0030 amendment 2).
use sacm::edit::{CopyElements, MoveElement};
// The create of slice 0057 (SACM23-ART-010, ADR 0034 amendment 2).
use sacm::edit::CreateArtifactAssetRelationship;
// The operations of slice 0062 (SACM23-TERM-001 to -006, ADR 0034 amendment 3).
use sacm::edit::{
    CreateCategory, CreateExpression, CreateTerm, CreateTerminologyGroup, CreateTerminologyPackage,
    SetExternalReference, SetValue,
};
// The creates of slice 0066 (SACM23-ARG-014, -015, SACM23-ART-012, -013; ADR 0039).
use sacm::edit::{
    CreateArgumentPackageBinding, CreateArgumentPackageInterface, CreateArtifactPackageBinding,
    CreateArtifactPackageInterface,
};
// The creates of slice 0067 (SACM23-AC-002, -003, SACM23-TERM-007, -008; ADR 0039).
use sacm::edit::{
    CreateAssuranceCasePackageBinding, CreateAssuranceCasePackageInterface,
    CreateTerminologyPackageBinding, CreateTerminologyPackageInterface,
};
use sacm::model::{
    AssertionDeclaration, Document, ElementId, ElementKind, Flag, LangString, MultiLangString,
    ReferenceEnd,
};
// The href of slice 0073b (LIB-EDIT-020; ADR 0040 amendment 1).
use sacm::model::Href;
use sacm::validation;
use sacm::xmi::{self, ReadOptions};
use serde::Serialize;
use serde::de::DeserializeOwned;

const API_006: &str = "LIB-API-006";
const EDIT_004: &str = "LIB-EDIT-004";
const EDIT_005: &str = "LIB-EDIT-005";
const EDIT_006: &str = "LIB-EDIT-006";
const EDIT_007: &str = "LIB-EDIT-007";
const EDIT_014: &str = "LIB-EDIT-014";
const EDIT_017: &str = "LIB-EDIT-017";
const EDIT_018: &str = "LIB-EDIT-018";
const ART_010: &str = "SACM23-ART-010";
const TERM_001: &str = "SACM23-TERM-001";
const TERM_004: &str = "SACM23-TERM-004";
const TERM_006: &str = "SACM23-TERM-006";
const ARG_014: &str = "SACM23-ARG-014";
const ARG_015: &str = "SACM23-ARG-015";
const ART_012: &str = "SACM23-ART-012";
const ART_013: &str = "SACM23-ART-013";
const AC_002: &str = "SACM23-AC-002";
const AC_003: &str = "SACM23-AC-003";
const TERM_007: &str = "SACM23-TERM-007";
const TERM_008: &str = "SACM23-TERM-008";

/// The directory of the golden JSON files.
const GOLDEN: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/tests/data/golden/serde");

/// Every operation name of this release: the creates first, in the order of
/// `operations.json`. `CopyElements` has no sample: it is not serialized
/// (ADR 0030 amendment 2). Slice 0062 adds five creates after
/// `CreateArtifactAssetRelationship`, and `SetValue` and
/// `SetExternalReference` after `SetEndTime` (ADR 0034 amendment 3). Slice
/// 0066 adds the interface and binding creates after `CreateArgumentPackage`
/// and `CreateArtifactPackage` (ADR 0039), and slice 0067 those of the
/// Assurance Case and Terminology packages after `CreateAssuranceCasePackage`
/// and `CreateTerminologyGroup`, after each package's existing samples.
const OPERATION_NAME_COUNT: usize = 57;
const OPERATION_NAMES: [&str; OPERATION_NAME_COUNT] = [
    "CreateAssuranceCasePackage",
    // The creates of slice 0067 (SACM23-AC-002, -003).
    "CreateAssuranceCasePackageInterface",
    "CreateAssuranceCasePackageBinding",
    "CreateArgumentPackage",
    // The creates of slice 0066 (SACM23-ARG-014, -015).
    "CreateArgumentPackageInterface",
    "CreateArgumentPackageBinding",
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
    "CreateArtifactPackage",
    // The creates of slice 0066 (SACM23-ART-012, -013).
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
    // The creates of slice 0062 (SACM23-TERM-001 to -006).
    "CreateTerminologyPackage",
    "CreateCategory",
    "CreateTerm",
    "CreateExpression",
    "CreateTerminologyGroup",
    // The creates of slice 0067 (SACM23-TERM-007, -008).
    "CreateTerminologyPackageInterface",
    "CreateTerminologyPackageBinding",
    "SetName",
    "SetGid",
    "SetFlag",
    "SetAssertionDeclaration",
    "SetContent",
    "SetKey",
    "SetVersion",
    "SetDate",
    "SetLocation",
    "SetStartTime",
    "SetEndTime",
    // The value edits of slice 0062 (SACM23-TERM-003, -004).
    "SetValue",
    "SetExternalReference",
    "SetReferences",
    "AddReference",
    "RemoveReference",
    "ReorderElements",
    "MoveElement",
    "DeleteElement",
];

/// The creates of [`OPERATION_NAMES`]: 38 with the five of slice 0062, the
/// four of slice 0066 and the four of slice 0067.
const CREATE_COUNT: usize = 38;

/// Every `EditError` variant of this release, with its catalog code, in the
/// order of `edit-errors.json`.
const EDIT_ERROR_COUNT: usize = 24;
const EDIT_ERRORS: [(&str, &str); EDIT_ERROR_COUNT] = [
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
    // The refusal of slice 0065 (SACM23-BASE-009).
    ("DerivedEnd", "SACM-CMD-008"),
    ("ReferenceNotHeld", "SACM-CMD-001"),
    // The refusal of slice 0073b (LIB-EDIT-020).
    ("HrefNotHeld", "SACM-CMD-001"),
    ("ChildNotFound", "SACM-CMD-001"),
    ("InvalidOrder", "SACM-CMD-009"),
    ("Referenced", "SACM-CMD-002"),
    ("PreviewExpired", "SACM-CMD-003"),
    ("NotEmpty", "SACM-CMD-006"),
    ("CrossPackageReference", "SACM-CMD-007"),
    // The refusals of slice 0056 (LIB-EDIT-017).
    ("InvalidRoot", "SACM-CMD-005"),
    ("ParentInSubtree", "SACM-CMD-005"),
    ("PositionOutOfRange", "SACM-CMD-009"),
    // The refusal of slice 0076 (LIB-COMPAT-019).
    ("KeptRootType", "SACM-CMD-005"),
];

/// Every catalog `Code` constant of this release. The catalog cross-check
/// keeps it in step with `docs/sacm/diagnostics.md`.
const CODES: &[Code] = &[
    Code::XML_MALFORMED,
    Code::XML_DOCTYPE_REJECTED,
    Code::XMI_INVALID_ROOT,
    Code::XMI_MISSING_ID,
    Code::XMI_MISSING_TYPE,
    Code::XMI_XSI_TYPE,
    Code::XMI_VERSION_IGNORED,
    Code::XMI_LEGACY_SHAPE,
    // The code of slice 0057 (SACM23-XMI-009).
    Code::XMI_SPELLING_VARIANT,
    Code::XMI_UNKNOWN_CONTENT,
    Code::XMI_STRICT_SAVE_REFUSED,
    Code::XMI_EXTERNAL_HREF,
    Code::XMI_UNREAD_CONTENT,
    Code::MULTIPLICITY_VIOLATION,
    Code::ENUM_INVALID_LITERAL,
    Code::CMD_TARGET_NOT_FOUND,
    Code::CMD_DELETE_REFERENCED,
    Code::CMD_PREVIEW_EXPIRED,
    Code::CMD_DUPLICATE_ID,
    Code::CMD_INVALID_PARENT,
    Code::CMD_PACKAGE_NOT_EMPTY,
    Code::CMD_EXTERNAL_REFERENCES,
    Code::CMD_FEATURE_NOT_APPLICABLE,
    Code::CMD_INVALID_ORDER,
    Code::ID_DUPLICATE,
    Code::ID_INVALID,
    Code::GID_DUPLICATE,
    Code::ID_TRIMMED,
    Code::PACKAGE_CONTENT_INVALID,
    // The code of slice 0068 (SACM23-VAL-023).
    Code::INTERFACE_PLACEMENT,
    Code::REF_DANGLING,
    Code::REF_WRONG_TYPE,
    Code::REF_PRESERVED_TARGET,
    Code::CITATION_INVALID,
    Code::AS_CITED_INVALID,
    Code::ABSTRACT_FORM_INVALID,
    // The codes of slice 0063 (LIB-API-017).
    Code::XML_ENCODING_NOT_READ,
    Code::XML_UNREPRESENTABLE_CHARACTER,
    Code::PARSER_LIMIT,
    Code::XMI_NAMESPACE_NOT_PINNED,
    Code::XMI_WRAPPER_NAMESPACE,
    Code::XMI_STRICT_SAVE_INVALID,
    // The code of slice 0065 (SACM23-VAL-009).
    Code::EXPRESSION_WITH_CONTENT,
];

/// [`CODES`], with the codes of slice 0072: SACM-SEC-003 (LIB-SEC-003) and
/// SACM-XMI-008 (LIB-COMPAT-004); and of slice 0075a: SACM-XMI-017
/// (LIB-COMPAT-002).
fn codes() -> Vec<Code> {
    let mut codes = CODES.to_vec();
    codes.extend([
        Code::LIMIT_EXCEEDED,
        Code::XMI_OTHER_REVISION,
        Code::XMI_FRAGMENT_PATH,
    ]);
    codes
}

/// The three flags, with their names.
const FLAGS: [(Flag, &str); 3] = [
    (Flag::IsCitation, "isCitation"),
    (Flag::IsAbstract, "isAbstract"),
    (Flag::IsCounter, "isCounter"),
];

/// The reference ends, with their XMI names and their serde names: the ten
/// of slice 0056, whose serde name is the XMI name, the two of slice 0057,
/// which share their XMI names with an `AssertedRelationship`'s ends and are
/// serialized as `artifactSource` and `artifactTarget` (ADR 0034 amendment
/// 2), and the four of slice 0062, whose serde name is the XMI name (ADR 0034
/// amendment 3).
const ENDS: &[(ReferenceEnd, &str, &str)] = &[
    (ReferenceEnd::CitedElement, "citedElement", "citedElement"),
    (ReferenceEnd::AbstractForm, "abstractForm", "abstractForm"),
    (ReferenceEnd::MetaClaim, "metaClaim", "metaClaim"),
    (ReferenceEnd::Reasoning, "reasoning", "reasoning"),
    (ReferenceEnd::Source, "source", "source"),
    (ReferenceEnd::Target, "target", "target"),
    (ReferenceEnd::Structure, "structure", "structure"),
    (
        ReferenceEnd::ReferencedArtifactElement,
        "referencedArtifactElement",
        "referencedArtifactElement",
    ),
    (
        ReferenceEnd::GroupMember,
        "argumentElement",
        "argumentElement",
    ),
    (
        ReferenceEnd::ArtifactGroupMember,
        "artifactElement",
        "artifactElement",
    ),
    (ReferenceEnd::ArtifactSource, "source", "artifactSource"),
    (ReferenceEnd::ArtifactTarget, "target", "artifactTarget"),
    (ReferenceEnd::Category, "category", "category"),
    (ReferenceEnd::Element, "element", "element"),
    (ReferenceEnd::Origin, "origin", "origin"),
    (
        ReferenceEnd::TerminologyGroupMember,
        "terminologyElement",
        "terminologyElement",
    ),
    // The ends of slice 0066 (ADR 0039 D1), whose serde name is the XMI name.
    (ReferenceEnd::Interface, "interface", "interface"),
    (ReferenceEnd::Implements, "implements", "implements"),
    (
        ReferenceEnd::ParticipantPackage,
        "participantPackage",
        "participantPackage",
    ),
    // The derived end of slice 0065 (SACM23-BASE-009).
    (ReferenceEnd::Expression, "expression", "expression"),
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

/// The document the `KeptRootType` sample is refused on (slice 0076,
/// LIB-COMPAT-019): the AssuranceCasePackage root `case` and the
/// ArgumentPackage root `a`, which keeps `xmi:type="p:Bundle"`.
const KEPT_TYPE_START: &str = r#"<xmi:XMI xmlns:sacm="http://www.omg.org/spec/SACM/20220301" xmlns:xmi="http://www.omg.org/spec/XMI/20131001" xmlns:p="urn:example:profile">
  <sacm:AssuranceCasePackage xmi:id="case"><name content="Case"/></sacm:AssuranceCasePackage>
  <sacm:ArgumentPackage xmi:type="p:Bundle" xmi:id="a"><name content="A"/></sacm:ArgumentPackage>
</xmi:XMI>
"#;

/// The document the delete samples are taken on. Three `ArgumentPackage`s:
/// - `p`: the Claims `c1`, `c2` (meta-claim `c1`), `c4` and `c6`, and the
///   `ArgumentGroup` `g1` (member `c6`);
/// - `q`: the Claim `c3` (meta-claims `c4` and `c6`, across the package
///   boundary);
/// - `r`: the Claim `c5`, its only element.
///
/// So deleting `c1` dangles one reference inside `p` (CMD-002), deleting `c4`
/// one across it (CMD-007), deleting `r` meets one contained element
/// (CMD-006), and deleting `c6` dangles one of each.
const DELETE_START: &str = r#"<sacm:AssuranceCasePackage xmlns:sacm="http://www.omg.org/spec/SACM/20220301" xmlns:xmi="http://www.omg.org/spec/XMI/20131001" xmi:id="case">
  <name content="Case"/>
  <argumentPackage xmi:id="p">
    <name content="P"/>
    <argumentElement xmi:type="sacm:Claim" xmi:id="c1"><name content="C1"/></argumentElement>
    <argumentElement xmi:type="sacm:Claim" xmi:id="c2" metaClaim="c1"><name content="C2"/></argumentElement>
    <argumentElement xmi:type="sacm:Claim" xmi:id="c4"><name content="C4"/></argumentElement>
    <argumentElement xmi:type="sacm:Claim" xmi:id="c6"><name content="C6"/></argumentElement>
    <argumentElement xmi:type="sacm:ArgumentGroup" xmi:id="g1" argumentElement="c6"><name content="G1"/></argumentElement>
  </argumentPackage>
  <argumentPackage xmi:id="q">
    <name content="Q"/>
    <argumentElement xmi:type="sacm:Claim" xmi:id="c3" metaClaim="c4 c6"><name content="C3"/></argumentElement>
  </argumentPackage>
  <argumentPackage xmi:id="r">
    <name content="R"/>
    <argumentElement xmi:type="sacm:Claim" xmi:id="c5"><name content="C5"/></argumentElement>
  </argumentPackage>
</sacm:AssuranceCasePackage>
"#;

/// The `PreviewExpired` sample of `edit-errors.json`: a revision depends on
/// the process, so the sample is read from JSON, not refused by a real stale
/// preview.
const PREVIEW_EXPIRED_JSON: &str =
    r#"{"type":"PreviewExpired","code":"SACM-CMD-003","previewed":1,"current":2}"#;

/// The three policy keys of `DeleteElement`, each with its default, the
/// reject variant, and its other variants (ADR 0031).
const DELETE_POLICIES: [(&str, &str, &[&str]); 3] = [
    (
        "reference_policy",
        "RejectIfReferenced",
        &["DeleteReferencingRelationships", "ScrubReferences"],
    ),
    ("package_policy", "RejectIfNonEmpty", &["DeleteRecursively"]),
    (
        "cross_package_policy",
        "RejectIfExternalReferencesExist",
        &["ApplyReferencePolicy"],
    ),
];

/// `DeleteElement::new("c1")` with every key written, compact.
const DELETE_JSON: &str = r#"{"type":"DeleteElement","element":"c1","reference_policy":"RejectIfReferenced","package_policy":"RejectIfNonEmpty","cross_package_policy":"RejectIfExternalReferencesExist"}"#;

/// The keys of a serialized `Preview`, in order (ADR 0031): the operation is
/// the mutation's, so it is written once, inside `mutation`.
const PREVIEW_KEYS: [&str; 3] = ["revision", "mutation", "dangling_references"];

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
    assert_golden_for(API_006, name, values);
}

/// [`assert_golden`], with messages that start with `row`.
fn assert_golden_for<T>(row: &str, name: &str, values: &T)
where
    T: Serialize + DeserializeOwned + PartialEq + Debug,
{
    let expected = golden(name);
    let written = match serde_json::to_string_pretty(values) {
        Ok(text) => text + "\n",
        Err(error) => panic!("{row}: the samples of {name} do not serialize: {error}"),
    };
    assert_eq!(
        written, expected,
        "{row}: serializing the samples does not give the bytes of {name}"
    );
    let read: T = match serde_json::from_str(&expected) {
        Ok(read) => read,
        Err(error) => panic!("{row}: {name} does not read back: {error}"),
    };
    assert_eq!(
        &read, values,
        "{row}: {name} reads back to values other than the samples"
    );
}

/// `valid` reads as a `T`, and `valid` with its first `from` replaced by
/// `to` is refused as data (not as malformed JSON): the refusal is caused by
/// the change, which `what` names.
fn assert_refused<T: DeserializeOwned + Debug>(what: &str, valid: &str, from: &str, to: &str) {
    assert_refused_for::<T>(API_006, what, valid, from, to);
}

/// [`assert_refused`], with messages that start with `row`.
fn assert_refused_for<T: DeserializeOwned + Debug>(
    row: &str,
    what: &str,
    valid: &str,
    from: &str,
    to: &str,
) {
    let invalid = valid.replacen(from, to, 1);
    assert_ne!(
        invalid, valid,
        "{row}: setup: {what}: {from:?} is not in {valid}"
    );
    if let Err(error) = serde_json::from_str::<T>(valid) {
        panic!("{row}: setup: {what}: the valid twin {valid} does not read: {error}");
    }
    match serde_json::from_str::<T>(&invalid) {
        Ok(value) => panic!("{row}: {what}: {invalid} was read as {value:?}"),
        Err(error) => assert!(
            error.is_data(),
            "{row}: {what}: {invalid} is refused as malformed JSON, not as data: {error}"
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

fn kept_type_start() -> Document {
    match xmi::read(KEPT_TYPE_START.as_bytes(), &ReadOptions::default()) {
        Ok(output) => output.into_document(),
        Err(error) => panic!("{API_006}: setup: KEPT_TYPE_START does not read: {error}"),
    }
}

fn delete_start() -> Document {
    match xmi::read(DELETE_START.as_bytes(), &ReadOptions::default()) {
        Ok(output) => output.into_document(),
        Err(error) => panic!("{EDIT_005}: setup: DELETE_START does not read: {error}"),
    }
}

/// `DeleteElement::new(element)` that scrubs every reference, across
/// packages too.
fn scrubbing_delete(element: &str) -> DeleteElement {
    DeleteElement::new(element)
        .with_reference_policy(ReferenceDeletePolicy::ScrubReferences)
        .with_cross_package_policy(CrossPackageReferencePolicy::ApplyReferencePolicy)
}

/// The preview of `operation`, which must be accepted, on `doc`.
fn previewed(doc: &Document, operation: impl Into<Operation>) -> Preview {
    let operation = operation.into();
    match edit::preview(doc, &operation) {
        Ok(preview) => preview,
        Err(error) => {
            panic!("{EDIT_004}: setup: the preview of {operation:?} was refused: {error}")
        }
    }
}

/// `apply_all`'s outcome, which must be accepted.
fn committed(result: Result<BatchMutation, BatchError>) -> BatchMutation {
    match result {
        Ok(batch) => batch,
        Err(error) => panic!("{EDIT_014}: setup: the batch was refused: {error}"),
    }
}

/// `apply_all`'s outcome, which must be a refusal.
fn batch_refusal(result: Result<BatchMutation, BatchError>) -> BatchError {
    match result {
        Ok(batch) => panic!("{EDIT_014}: setup: the batch was accepted: {batch:?}"),
        Err(error) => error,
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
    samples.extend(artifact_samples());
    samples.extend(edit_samples());
    samples.extend(delete_samples());
    samples.extend(href_samples());
    samples
}

/// The package and Claim creates of [`operation_samples`].
fn package_and_claim_samples() -> Vec<Operation> {
    let mut samples: Vec<Operation> = vec![
        CreateAssuranceCasePackage::root(en("Safety case")).into(),
        CreateAssuranceCasePackage::root(en("Safety case"))
            .with_id("case")
            .into(),
        CreateAssuranceCasePackage::new("case", en("Subsystem")).into(),
        CreateAssuranceCasePackage::new("case", en("Subsystem"))
            .with_id("sub")
            .into(),
    ];
    samples.extend(assurance_case_interface_samples());
    let rest: Vec<Operation> = vec![
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
    ];
    samples.extend(rest);
    samples
}

/// The Assurance Case creates of slice 0067 (SACM23-AC-002, -003; ADR 0039
/// D4), in the shapes of [`argument_interface_samples`].
fn assurance_case_interface_samples() -> Vec<Operation> {
    vec![
        CreateAssuranceCasePackageInterface::root(en("Braking case interface"), "braking_case")
            .with_id("braking_case_api")
            .into(),
        CreateAssuranceCasePackageInterface::new(
            "braking_case",
            en("Braking case interface"),
            "braking_case",
        )
        .into(),
        CreateAssuranceCasePackageBinding::root(
            en("The braking case supports the vehicle case"),
            ["braking_case_api", "vehicle_case_api"],
        )
        .with_id("binding")
        .into(),
        CreateAssuranceCasePackageBinding::new(
            "system",
            en("The braking case supports the vehicle case"),
            ["braking_case_api", "vehicle_case_api"],
        )
        .into(),
    ]
}

/// The Terminology creates of slice 0067 (SACM23-TERM-007, -008; ADR 0039
/// D4), in the shapes of [`argument_interface_samples`].
fn terminology_interface_samples() -> Vec<Operation> {
    vec![
        CreateTerminologyPackageInterface::root(en("Hazard terminology interface"), "hazard_terms")
            .with_id("hazard_terms_api")
            .into(),
        CreateTerminologyPackageInterface::new(
            "hazard_terms",
            en("Hazard terminology interface"),
            "hazard_terms",
        )
        .into(),
        CreateTerminologyPackageBinding::root(
            en("Hazard terms map to design terms"),
            ["hazard_terms_api", "design_terms_api"],
        )
        .with_id("binding")
        .into(),
        CreateTerminologyPackageBinding::new(
            "case",
            en("Hazard terms map to design terms"),
            ["hazard_terms_api", "design_terms_api"],
        )
        .into(),
    ]
}

/// The creates of [`operation_samples`] that reference other elements.
fn argumentation_samples() -> Vec<Operation> {
    let sources = ["c2", "c3"];
    let mut samples: Vec<Operation> = vec![
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
    ];
    samples.extend(argument_interface_samples());
    samples
}

/// The Argumentation creates of slice 0066 (SACM23-ARG-014, -015; ADR 0039
/// D4): an interface and a binding, each as a root with an id and under a
/// parent without one.
fn argument_interface_samples() -> Vec<Operation> {
    vec![
        CreateArgumentPackageInterface::root(en("Braking interface"), "braking")
            .with_id("braking_api")
            .into(),
        CreateArgumentPackageInterface::new("braking", en("Braking interface"), "braking").into(),
        CreateArgumentPackageBinding::root(
            en("Braking supports the vehicle"),
            ["braking_api", "vehicle_api"],
        )
        .with_id("binding")
        .into(),
        CreateArgumentPackageBinding::new(
            "case",
            en("Braking supports the vehicle"),
            ["braking_api", "vehicle_api"],
        )
        .into(),
    ]
}

/// The Artifact creates of slice 0066 (SACM23-ART-012, -013; ADR 0039 D4),
/// in the shapes of [`argument_interface_samples`].
fn artifact_interface_samples() -> Vec<Operation> {
    vec![
        CreateArtifactPackageInterface::root(en("Brake tests interface"), "tests")
            .with_id("tests_api")
            .into(),
        CreateArtifactPackageInterface::new("tests", en("Brake tests interface"), "tests").into(),
        CreateArtifactPackageBinding::root(
            en("The tests verify the requirement"),
            ["tests_api", "requirements_api"],
        )
        .with_id("binding")
        .into(),
        CreateArtifactPackageBinding::new(
            "case",
            en("The tests verify the requirement"),
            ["tests_api", "requirements_api"],
        )
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

/// The artifact creates of [`operation_samples`] (SACM23-ART-001, -003, -008,
/// -009, -011; ADR 0034): each with a generated and a caller id, the
/// `ArtifactPackage` as a root and under a parent, the `ArtifactGroup` without
/// and with members.
fn artifact_samples() -> Vec<Operation> {
    let mut samples: Vec<Operation> = vec![
        CreateArtifactPackage::root(en("Evidence")).into(),
        CreateArtifactPackage::root(en("Evidence"))
            .with_id("evidence")
            .into(),
        CreateArtifactPackage::new("case", en("Evidence")).into(),
        CreateArtifactPackage::new("evidence", en("Test evidence"))
            .with_id("tests")
            .into(),
        CreateProperty::new("t1", en("Qualified")).into(),
        CreateProperty::new("t1", en("Qualified"))
            .with_id("pr1")
            .into(),
        CreateTechnique::new("evidence", en("Fault tree analysis")).into(),
        CreateTechnique::new("evidence", en("Fault tree analysis"))
            .with_id("t1")
            .into(),
        CreateParticipant::new("evidence", en("Safety assessor")).into(),
        CreateParticipant::new("evidence", en("Safety assessor"))
            .with_id("pa1")
            .into(),
        CreateArtifactGroup::new("evidence", en("Analysis")).into(),
        CreateArtifactGroup::new("evidence", en("Analysis"))
            .with_id("ag1")
            .with_group_members(["t1", "pa1"])
            .into(),
        // The creates of slice 0055 (SACM23-ART-004 to -007; ADR 0034
        // amendment 1): each without and with its id and values.
        CreateArtifact::new("evidence", en("Brake test report")).into(),
        CreateArtifact::new("evidence", en("Brake test report"))
            .with_id("report")
            .with_version("1.2")
            .with_date("2024-03-01")
            .into(),
        CreateEvent::new("evidence", en("Report released")).into(),
        CreateEvent::new("evidence", en("Report released"))
            .with_id("released")
            .with_date("2024-03-05")
            .into(),
        CreateResource::new("evidence", en("Report PDF")).into(),
        CreateResource::new("evidence", en("Report PDF"))
            .with_id("report_pdf")
            .with_location(en("https://example.org/r.pdf"))
            .into(),
        CreateActivity::new("evidence", en("Brake testing")).into(),
        CreateActivity::new("evidence", en("Brake testing"))
            .with_id("testing")
            .with_start_time("2024-02-12")
            .with_end_time("2024-02-29T17:30:00Z")
            .into(),
    ];
    // The create of slice 0057 (SACM23-ART-010; ADR 0034 amendment 2):
    // without and with its id, the second with two sources and two targets.
    samples.extend([
        Operation::from(CreateArtifactAssetRelationship::new(
            "evidence",
            en("Produced by"),
            ["report"],
            ["testing"],
        )),
        CreateArtifactAssetRelationship::new(
            "evidence",
            en("Produced by"),
            ["report", "report_pdf"],
            ["testing", "tester"],
        )
        .with_id("rel1")
        .into(),
    ]);
    samples.extend(artifact_interface_samples());
    samples.extend(terminology_samples());
    samples
}

/// The creates of slice 0062 (SACM23-TERM-001 to -006; ADR 0034 amendment
/// 3), each without and with its id and values: a TerminologyPackage as a
/// root and in `case`, then a Category, a Term, an Expression and a
/// `TerminologyGroup` in `terms`; then the interface and binding creates of
/// slice 0067.
fn terminology_samples() -> Vec<Operation> {
    let mut samples: Vec<Operation> = vec![
        CreateTerminologyPackage::root(en("Blender terminology")).into(),
        CreateTerminologyPackage::new("case", en("Blender terminology"))
            .with_id("terms")
            .into(),
        CreateCategory::new("terms", en("Hazards")).into(),
        CreateCategory::new("terms", en("Mechanical hazards"))
            .with_id("mechanical")
            .with_categories(["hazards"])
            .into(),
        CreateTerm::new("terms", en("Blade"), "blade").into(),
        CreateTerm::new("terms", en("Blade"), "blade")
            .with_id("blade")
            .with_categories(["mechanical"])
            .with_origin("manual")
            .with_external_reference("https://example.org/terms/blade")
            .into(),
        CreateExpression::new(
            "terms",
            en("Blade guarded by lid"),
            "$Blade$ is covered by the $Lid$",
        )
        .into(),
        CreateExpression::new(
            "terms",
            en("Blade guarded by lid"),
            "$Blade$ is covered by the $Lid$",
        )
        .with_id("blade_guarded")
        .with_categories(["mechanical"])
        .with_expression_elements(["blade", "lid"])
        .into(),
        CreateTerminologyGroup::new("terms", en("Reviewed by the safety team")).into(),
        CreateTerminologyGroup::new("terms", en("Reviewed by the safety team"))
            .with_id("reviewed")
            .with_group_members(["blade", "lid"])
            .into(),
    ];
    samples.extend(terminology_interface_samples());
    samples
}

/// The operations of [`operation_samples`] that change an element.
fn edit_samples() -> Vec<Operation> {
    let mut samples: Vec<Operation> = vec![
        SetName::new("c1", en("The system is safe enough")).into(),
        SetGid::new("c1", "G-1").into(),
        SetGid::remove("c1").into(),
        SetFlag::new("c1", Flag::IsCitation, true).into(),
        SetFlag::new("i1", Flag::IsCounter, false).into(),
        SetAssertionDeclaration::new("c1", AssertionDeclaration::NeedsSupport).into(),
        SetContent::new("d1", two_values()).into(),
        SetContent::remove("d1").into(),
        SetKey::new("t1", en("SIL")).into(),
        // The value edits of slice 0055, each set and removed.
        SetVersion::new("report", "1.3").into(),
        SetVersion::remove("report").into(),
        SetDate::new("released", "2024-03-06").into(),
        SetDate::remove("released").into(),
        SetLocation::new("report_pdf", two_values()).into(),
        SetLocation::remove("report_pdf").into(),
        SetStartTime::new("testing", "2024-02-12").into(),
        SetStartTime::remove("testing").into(),
        SetEndTime::new("testing", "2024-02-29T17:30:00Z").into(),
        SetEndTime::remove("testing").into(),
    ];
    // The value edits of slice 0062 (SACM23-TERM-003, -004), after SetEndTime.
    samples.extend([
        Operation::from(SetValue::new("blade", "blade")),
        SetExternalReference::new("blade", "https://example.org/terms/blade").into(),
        SetExternalReference::remove("blade").into(),
    ]);
    samples.extend([
        Operation::from(SetReferences::new("i1", ReferenceEnd::Source, ["c2", "c3"])),
        SetReferences::clear("c1", ReferenceEnd::MetaClaim).into(),
    ]);
    // The ends of slice 0057 (SACM23-ART-010), after the last sample of each
    // operation.
    samples.push(
        SetReferences::new("rel1", ReferenceEnd::ArtifactTarget, ["testing", "tester"]).into(),
    );
    // The ends of slice 0062 (SACM23-TERM-004, -005), after the rel1 sample.
    samples.extend([
        Operation::from(SetReferences::new(
            "blade_guarded",
            ReferenceEnd::Element,
            ["blade", "lid"],
        )),
        SetReferences::new("blade", ReferenceEnd::Origin, ["manual"]).into(),
    ]);
    samples.extend([
        Operation::from(AddReference::new("g1", ReferenceEnd::GroupMember, "c3")),
        AddReference::new("ag1", ReferenceEnd::ArtifactGroupMember, "pa1").into(),
    ]);
    // The ends of slice 0062 (SACM23-TERM-002, -006), after the ag1 sample.
    samples.extend([
        Operation::from(AddReference::new(
            "reviewed",
            ReferenceEnd::TerminologyGroupMember,
            "appliance",
        )),
        AddReference::new("lid", ReferenceEnd::Category, "hazards").into(),
    ]);
    samples.push(AddReference::new("rel1", ReferenceEnd::ArtifactSource, "report_pdf").into());
    // The end of slice 0066 (SACM23-ARG-015), after the rel1 sample.
    samples
        .push(AddReference::new("binding", ReferenceEnd::ParticipantPackage, "vehicle_api").into());
    samples.extend([
        Operation::from(RemoveReference::new("i1", ReferenceEnd::Source, "c3")),
        ReorderElements::new("arguments", ["c2", "c1"]).into(),
        ReorderElements::roots(["case", "arguments"]).into(),
    ]);
    // The moves of slice 0056 (LIB-EDIT-017): appended, at a position, and
    // to the roots at a position.
    samples.extend(move_samples());
    samples
}

/// The moves of [`edit_samples`]: `new`, `with_position` and `root`.
fn move_samples() -> Vec<Operation> {
    vec![
        MoveElement::new("c1", "evidence").into(),
        MoveElement::new("c2", "arguments").with_position(0).into(),
        MoveElement::root("sub").with_position(1).into(),
    ]
}

/// The deletes of [`operation_samples`]: with the default policies, with one
/// set, and with every policy set to its other variant, so that each of the
/// seven policy variants is written.
fn delete_samples() -> Vec<Operation> {
    vec![
        DeleteElement::new("c1").into(),
        DeleteElement::new("c1")
            .with_reference_policy(ReferenceDeletePolicy::DeleteReferencingRelationships)
            .into(),
        DeleteElement::new("p")
            .with_reference_policy(ReferenceDeletePolicy::ScrubReferences)
            .with_package_policy(PackageDeletePolicy::DeleteRecursively)
            .with_cross_package_policy(CrossPackageReferencePolicy::ApplyReferencePolicy)
            .into(),
    ]
}

/// Four mutations, as `edit::apply` returns them: a create with a generated
/// id, a flag set, a create with a caller id, and its delete; then a move of
/// slice 0056. In the order of `mutations.json`.
fn mutation_samples() -> Vec<Mutation> {
    let mut doc = Document::new();
    let mut samples = vec![
        applied(&mut doc, CreateArgumentPackage::root(en("Arguments"))),
        applied(
            &mut doc,
            SetFlag::new("argument_package_1", Flag::IsAbstract, true),
        ),
        applied(
            &mut doc,
            CreateClaim::new("argument_package_1", LangString::new("C1")).with_id("c1"),
        ),
        applied(&mut doc, DeleteElement::new("c1")),
    ];
    // A move of slice 0056 (LIB-EDIT-017): c2 into the root q.
    {
        applied(
            &mut doc,
            CreateArgumentPackage::root(LangString::new("Q")).with_id("q"),
        );
        applied(
            &mut doc,
            CreateClaim::new("argument_package_1", LangString::new("C2")).with_id("c2"),
        );
        samples.push(applied(&mut doc, MoveElement::new("c2", "q")));
    }
    samples
}

/// Every `EditError` variant, as `edit::apply` refuses on [`ERROR_START`],
/// with both forms of `ChildNotFound` and `InvalidOrder`, and a second
/// `WrongReferenceType` and `FeatureNotApplicable`; then the delete refusals
/// on [`DELETE_START`] and the sample [`PREVIEW_EXPIRED_JSON`]. In the order
/// of `edit-errors.json`.
fn edit_error_samples() -> Vec<EditError> {
    let mut samples = edit_refusal_samples();
    let deletes = delete_start();
    samples.extend([
        refusal(&deletes, DeleteElement::new("c1")),
        from_json(PREVIEW_EXPIRED_JSON),
        refusal(&deletes, DeleteElement::new("r")),
        refusal(&deletes, DeleteElement::new("c4")),
    ]);
    // A value edit of slice 0055 on a kind without the value (SACM23-ART-002).
    samples.push(refusal(
        &artifact_assets_start(),
        SetDate::new("testing", "2024-03-06"),
    ));
    // The other value edits of slice 0055, each on a kind without the value,
    // so each feature name reads back (SACM23-ART-004, -005, -006).
    let assets = artifact_assets_start();
    samples.extend([
        refusal(&assets, SetVersion::new("released", "1.3")),
        refusal(&assets, SetLocation::new("report", en("x"))),
        refusal(&assets, SetStartTime::new("report", "t")),
        refusal(&assets, SetEndTime::new("report", "t")),
    ]);
    // An AssertedRelationship's end on an ArtifactAssetRelationship (slice
    // 0057, SACM23-ART-010): the JSON names the feature by its XMI name.
    samples.push(refusal(
        &artifact_relationship_start(),
        SetReferences::new("rel1", ReferenceEnd::Source, ["report"]),
    ));
    // A feature of slice 0062 on a kind without it, one per feature name
    // (SACM23-TERM-002 to -006), after the rel1 source sample.
    {
        let terms = terminology_start();
        samples.extend([
            refusal(&terms, SetValue::new("hazards", "x")),
            refusal(&terms, SetExternalReference::new("blade_guarded", "u")),
            refusal(
                &terms,
                SetReferences::new("c1", ReferenceEnd::Category, ["hazards"]),
            ),
            refusal(
                &terms,
                SetReferences::new("blade", ReferenceEnd::Element, ["lid"]),
            ),
            refusal(
                &terms,
                SetReferences::new("blade_guarded", ReferenceEnd::Origin, ["c1"]),
            ),
            refusal(
                &terms,
                SetReferences::new("ag1", ReferenceEnd::TerminologyGroupMember, ["blade"]),
            ),
        ]);
    }
    // The refusals of slice 0056 (LIB-EDIT-017), on ERROR_START.
    {
        let doc = error_start();
        samples.extend([
            refusal(&doc, MoveElement::root("c1")),
            refusal(&doc, MoveElement::new("p", "p")),
            refusal(&doc, MoveElement::new("c1", "p").with_position(2)),
            refusal(&doc, MoveElement::root("case").with_position(1)),
        ]);
    }
    // The refusal of slice 0076 (LIB-COMPAT-019), on KEPT_TYPE_START.
    samples.push(refusal(&kept_type_start(), MoveElement::new("a", "case")));
    samples
}

/// [`ERROR_START`] with the TerminologyPackage `terms` under `case`, holding
/// the Category `hazards`, the Terms `blade` and `lid` and the Expression
/// `blade_guarded`, and the ArtifactPackage `evidence` holding the
/// `ArtifactGroup` `ag1` (slice 0062).
fn terminology_start() -> Document {
    let mut doc = error_start();
    for operation in [
        Operation::from(CreateTerminologyPackage::new("case", en("Terms")).with_id("terms")),
        CreateCategory::new("terms", en("Hazards"))
            .with_id("hazards")
            .into(),
        CreateTerm::new("terms", en("Blade"), "blade")
            .with_id("blade")
            .into(),
        CreateTerm::new("terms", en("Lid"), "lid")
            .with_id("lid")
            .into(),
        CreateExpression::new("terms", en("Blade guarded"), "$Blade$ guarded")
            .with_id("blade_guarded")
            .into(),
        CreateArtifactPackage::new("case", en("Evidence"))
            .with_id("evidence")
            .into(),
        CreateArtifactGroup::new("evidence", en("Analysis"))
            .with_id("ag1")
            .into(),
    ] {
        applied(&mut doc, operation);
    }
    doc
}

/// The worked fixture of slice 0055, read.
fn artifact_assets_start() -> Document {
    let path = concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/tests/data/sacm23/xmi/artifact-assets-valid.sacm.xmi"
    );
    let bytes = std::fs::read(path)
        .unwrap_or_else(|error| panic!("{API_006}: setup: cannot read {path}: {error}"));
    match xmi::read(&bytes, &ReadOptions::default()) {
        Ok(output) => output.into_document(),
        Err(error) => {
            panic!("{API_006}: setup: the artifact assets fixture does not read: {error}")
        }
    }
}

/// The samples of [`edit_error_samples`] refused on [`ERROR_START`].
fn edit_refusal_samples() -> Vec<EditError> {
    let doc = error_start();
    let name = || LangString::new("N");
    // The refusals of slice 0057 (SACM23-ART-010), each after the last sample
    // of its variant.
    let relationships = artifact_relationship_start();
    let mut samples = vec![
        refusal(&doc, CreateClaim::new("nowhere", name())),
        refusal(&doc, CreateClaim::new("case", name())),
        refusal(&doc, CreateClaim::new("p", name()).with_id("1bad")),
        refusal(&doc, CreateClaim::new("p", name()).with_id("c1")),
        refusal(
            &doc,
            CreateAssertedInference::new("p", name(), Vec::<ElementId>::new(), "c1"),
        ),
    ];
    samples.push(refusal(
        &relationships,
        CreateArtifactAssetRelationship::new(
            "evidence",
            name(),
            ["report"],
            Vec::<ElementId>::new(),
        ),
    ));
    // The refusal of slice 0066 (SACM23-ARG-015): one distinct participant.
    samples.push(refusal(
        &doc,
        CreateArgumentPackageBinding::new("case", name(), ["p", "p"]),
    ));
    samples.extend([
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
    ]);
    samples.push(refusal(
        &relationships,
        CreateArtifactAssetRelationship::new("evidence", name(), ["report"], ["c1"]),
    ));
    samples.extend([
        refusal(&doc, SetName::new("nowhere", name())),
        refusal(&doc, SetFlag::new("c1", Flag::IsCounter, true)),
        refusal(&doc, SetReferences::new("c1", ReferenceEnd::Source, ["c1"])),
        // The refusal of slice 0065, after FeatureNotApplicable
        // (SACM23-BASE-009, ADR 0038 D3).
        refusal(
            &doc,
            SetReferences::new("c1", ReferenceEnd::Expression, ["blade"]),
        ),
        refusal(
            &doc,
            RemoveReference::new("c1", ReferenceEnd::MetaClaim, "m1"),
        ),
        // The refusal of slice 0073b, after ReferenceNotHeld (LIB-EDIT-020).
        href_not_held_sample(),
        refusal(&doc, ReorderElements::new("p", ["case"])),
        refusal(&doc, ReorderElements::roots(["c1"])),
        refusal(&doc, ReorderElements::new("p", Vec::<ElementId>::new())),
        refusal(&doc, ReorderElements::new("p", ["c1", "c1"])),
    ]);
    samples
}

/// The `HrefNotHeld` sample of `edit-errors.json` (LIB-EDIT-020): removing
/// an href that the `participantPackage` of a binding of two remote
/// interfaces does not hold.
fn href_not_held_sample() -> EditError {
    let input = r#"<sacm:ArgumentPackageBinding xmlns:sacm="http://www.omg.org/spec/SACM/20220301" xmlns:xmi="http://www.omg.org/spec/XMI/20131001" xmi:id="binding">
  <name content="Binding"/>
  <participantPackage href="braking.sacm.xmi#braking_api"/>
  <participantPackage href="vehicle.sacm.xmi#vehicle_api"/>
</sacm:ArgumentPackageBinding>
"#;
    let doc = match xmi::read(input.as_bytes(), &ReadOptions::default()) {
        Ok(output) => output.into_document(),
        Err(error) => panic!("LIB-EDIT-020: setup: the binding does not read: {error}"),
    };
    refusal(
        &doc,
        RemoveReference::href("binding", ReferenceEnd::ParticipantPackage, href("o.xmi#x")),
    )
}

/// The `Href` `value`, which `Href::new` must accept.
fn href(value: &str) -> Href {
    match Href::new(value) {
        Ok(href) => href,
        Err(error) => panic!("LIB-EDIT-020: setup: Href::new({value:?}) refused it: {error}"),
    }
}

/// The operations of slice 0073b that hold an href (LIB-EDIT-020; ADR 0040
/// amendment 1, A7), the last samples of `operations.json`: a set, an add and
/// a remove on a binding, an interface and a binding create, and a Claim
/// whose name's expression is in another document.
fn href_samples() -> Vec<Operation> {
    let remote = href("braking.sacm.xmi#braking_api");
    let participant = ReferenceEnd::ParticipantPackage;
    vec![
        SetReferences::new("binding", participant, ["vehicle_api"])
            .with_hrefs([remote.clone()])
            .into(),
        AddReference::href("binding", participant, remote.clone()).into(),
        RemoveReference::href("binding", participant, remote.clone()).into(),
        CreateArgumentPackageInterface::root_href(
            en("Braking interface"),
            href("braking.sacm.xmi#braking"),
        )
        .with_id("braking_api")
        .into(),
        CreateArgumentPackageBinding::root(en("Braking supports the vehicle"), ["vehicle_api"])
            .with_participant_package_hrefs([remote])
            .with_id("binding")
            .into(),
        CreateClaim::new(
            "arguments",
            LangString::new("x").with_expression_href(href("terms.sacm.xmi#blade")),
        )
        .into(),
    ]
}

/// [`ERROR_START`] with the `ArtifactPackage` `evidence` under `case`, holding
/// the Artifact `report`, the Activity `testing` and the
/// `ArtifactAssetRelationship` `rel1` from `report` to `testing`
/// (SACM23-ART-010).
fn artifact_relationship_start() -> Document {
    let mut doc = error_start();
    for operation in [
        Operation::from(CreateArtifactPackage::new("case", en("Evidence")).with_id("evidence")),
        CreateArtifact::new("evidence", en("Brake test report"))
            .with_id("report")
            .into(),
        CreateActivity::new("evidence", en("Brake testing"))
            .with_id("testing")
            .into(),
        CreateArtifactAssetRelationship::new(
            "evidence",
            en("Produced by"),
            ["report"],
            ["testing"],
        )
        .with_id("rel1")
        .into(),
    ] {
        applied(&mut doc, operation);
    }
    doc
}

/// A full diagnostic (clause, two affected ids, location) and a minimal one
/// (none of them), both built with `test-support`'s `new`, so with their
/// code's text as their template and no arguments; and, of slice 0064, the
/// one finding of a real `validate`, which carries its template and
/// arguments (LIB-API-018, acceptances 2 and 5). In the order of
/// `diagnostics.json`.
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
        two_descriptions_finding(),
    ]
}

/// The one finding of `validate` for the Claim `CLM_132` with two
/// descriptions: a SACM-MULT-001 Warning of the template
/// `SACM-MULT-001/too-many` (LIB-API-018, acceptance 2).
fn two_descriptions_finding() -> Diagnostic {
    const INPUT: &str = concat!(
        "<sacm:ArgumentPackage xmlns:sacm=\"http://www.omg.org/spec/SACM/20220301\" ",
        "xmlns:xmi=\"http://www.omg.org/spec/XMI/20131001\" xmi:id=\"p\"><name content=\"P\"/>\n",
        "  <argumentElement xmi:type=\"sacm:Claim\" xmi:id=\"CLM_132\"><name content=\"C\"/>",
        "<description xmi:id=\"d1\"/><description xmi:id=\"d2\"/></argumentElement>\n",
        "</sacm:ArgumentPackage>",
    );
    let document = match xmi::read(INPUT.as_bytes(), &ReadOptions::default()) {
        Ok(output) => output.into_document(),
        Err(error) => panic!("{API_006}: setup: {INPUT} does not read: {error}"),
    };
    let mut found = validation::validate(&document);
    assert_eq!(
        found.len(),
        1,
        "LIB-API-018: setup: CLM_132 gives one finding: {found:?}"
    );
    found.remove(0)
}

/// The values of `values.json`: a `LangString` absent, with empty content,
/// lang-only, full, and with non-ASCII and quoted content; a
/// `MultiLangString` of two values, empty, and of one absent value; a
/// `SourceLocation`; an `ElementId`; and, of slice 0065, an
/// `ExpressionLangString` and a `MultiLangString` holding one (ADR 0030
/// amendment 4).
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
    LangString,
    MultiLangString,
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
        LangString::default()
            .with_lang("en")
            .with_expression("blade"),
        [
            LangString::new("Kontrollerad mot manualen").with_lang("sv"),
            LangString::new("").with_lang("en").with_expression("blade"),
        ]
        .into_iter()
        .collect(),
    )
}

/// Three batches, as `edit::apply_all` commits them: creates with a caller
/// and a generated id and a flag set; a create and a delete of `tmp`, a
/// rename and a delete, whose net ids name `tmp` nowhere; and an empty batch.
/// In the order of `batch-mutations.json`.
fn batch_mutation_samples() -> Vec<BatchMutation> {
    let first: [Operation; 3] = [
        CreateArgumentPackage::root(en("Arguments"))
            .with_id("p")
            .into(),
        CreateClaim::new("p", LangString::new("C1")).into(),
        SetFlag::new("claim_1", Flag::IsAbstract, true).into(),
    ];
    let second: [Operation; 4] = [
        CreateClaim::new("q", LangString::new("Temp"))
            .with_id("tmp")
            .into(),
        DeleteElement::new("tmp").into(),
        SetName::new("c3", LangString::new("Renamed")).into(),
        DeleteElement::new("c5").into(),
    ];
    vec![
        committed(edit::apply_all(&mut Document::new(), first)),
        committed(edit::apply_all(&mut delete_start(), second)),
        committed(edit::apply_all(
            &mut Document::new(),
            Vec::<Operation>::new(),
        )),
    ]
}

/// Two refused batches: a create under a missing parent after an accepted
/// create, and a delete of a referenced element at index 0. In the order of
/// `batch-errors.json`.
fn batch_error_samples() -> Vec<BatchError> {
    let first: [Operation; 2] = [
        CreateArgumentPackage::root(en("Arguments"))
            .with_id("p")
            .into(),
        CreateClaim::new("nowhere", LangString::new("C1")).into(),
    ];
    vec![
        batch_refusal(edit::apply_all(&mut Document::new(), first)),
        batch_refusal(edit::apply_all(
            &mut delete_start(),
            [DeleteElement::new("c1")],
        )),
    ]
}

/// Two previews on [`DELETE_START`]: a scrubbing delete of `c6`, which
/// dangles a group member inside its package and a meta-claim across it, and
/// a create, which dangles nothing. In the order of `previews.json`.
fn preview_samples() -> Vec<Preview> {
    let doc = delete_start();
    vec![
        previewed(&doc, scrubbing_delete("c6")),
        previewed(
            &doc,
            CreateClaim::new("p", LangString::new("C7")).with_id("c7"),
        ),
    ]
}

/// The dangling references of the delete preview of [`preview_samples`]. In
/// the order of `dangling-references.json`.
fn dangling_reference_samples() -> Vec<DanglingReference> {
    previewed(&delete_start(), scrubbing_delete("c6"))
        .dangling_references()
        .to_vec()
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
    assert_serde::<CreateArtifactPackage>();
    assert_serde::<CreateProperty>();
    assert_serde::<CreateTechnique>();
    assert_serde::<CreateParticipant>();
    assert_serde::<CreateArtifactGroup>();
    {
        assert_serde::<CreateArtifact>();
        assert_serde::<CreateEvent>();
        assert_serde::<CreateResource>();
        assert_serde::<CreateActivity>();
        assert_serde::<SetVersion>();
        assert_serde::<SetDate>();
        assert_serde::<SetLocation>();
        assert_serde::<SetStartTime>();
        assert_serde::<SetEndTime>();
    }
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

#[test]
fn sacm23_term_001_the_terminology_operations_implement_serialize_and_deserialize() {
    assert_serde::<CreateTerminologyPackage>();
    assert_serde::<CreateCategory>();
    assert_serde::<CreateTerm>();
    assert_serde::<CreateExpression>();
    assert_serde::<CreateTerminologyGroup>();
    assert_serde::<SetValue>();
    assert_serde::<SetExternalReference>();
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
    assert_bare_for(API_006, value);
}

/// [`assert_bare`], with messages that start with `row`.
fn assert_bare_for<T>(row: &str, value: &T)
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
        "{row}: the bare {value:?} is not its Operation without \"type\""
    );
    let read: T = from_json(&bare);
    assert_eq!(&read, value, "{row}: the bare {bare} does not read back");
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
    assert_bare(&CreateArtifactPackage::root(name()));
    assert_bare(&CreateArtifactPackage::new("case", name()).with_id("e"));
    assert_bare(&CreateProperty::new("t1", name()));
    assert_bare(&CreateTechnique::new("e", name()).with_id("t1"));
    assert_bare(&CreateParticipant::new("e", name()));
    assert_bare(&CreateArtifactGroup::new("e", name()).with_group_members(["t1"]));
    {
        assert_bare(&CreateArtifact::new("e", name()).with_version("1.2"));
        assert_bare(&CreateEvent::new("e", name()).with_date(""));
        assert_bare(&CreateResource::new("e", name()).with_location(two_values()));
        assert_bare(&CreateActivity::new("e", name()).with_end_time("2024"));
        assert_bare(&SetVersion::remove("r"));
        assert_bare(&SetDate::new("e1", "2024-03-06"));
        assert_bare(&SetLocation::remove("r"));
        assert_bare(&SetStartTime::new("a1", "2024"));
        assert_bare(&SetEndTime::remove("a1"));
    }
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
    // Every end, in `ReferenceEnd::ALL` order.
    assert_eq!(
        ENDS.iter().map(|&(end, _, _)| end).collect::<Vec<_>>(),
        ReferenceEnd::ALL,
        "{API_006}: setup: the ends listed here are not ReferenceEnd::ALL"
    );
    for &(end, xmi_name, serde_name) in ENDS {
        assert_eq!(
            end.name(),
            xmi_name,
            "{API_006}: setup: the name of {end:?}"
        );
        assert_name_round_trips(end, serde_name);
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
    let codes = codes();
    let constants: BTreeSet<&str> = codes.iter().copied().map(Code::as_str).collect();
    assert_eq!(
        constants, live,
        "{API_006}: setup: the codes listed here are not the live codes of the catalog"
    );
    for &code in &codes {
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
    // SACM23-ART-011: an unknown key in a CreateArtifactGroup, such as a
    // `members` for `group_members` (ADR 0034).
    assert_refused_for::<Operation>(
        "SACM23-ART-011",
        "members in a CreateArtifactGroup",
        r#"{"type":"CreateArtifactGroup","parent":"p","id":null,"name":{},"group_members":["t1"]}"#,
        r#""group_members""#,
        r#""members""#,
    );
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
    // SACM23-ART-001: `parent` is required, `null` for a root (ADR 0034).
    let artifacts = r#"{"type":"CreateArtifactPackage","parent":null,"name":{"content":"N"}}"#;
    assert_refused_for::<Operation>(
        "SACM23-ART-001",
        "an ArtifactPackage create without parent",
        artifacts,
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
fn lib_api_006_an_omitted_optional_key_of_an_artifact_create_reads_as_the_constructor_default() {
    // The creates of slice 0052 (ADR 0034): `id` reads as none and
    // `group_members` as empty; `parent` is required, `null` for a root.
    let name = || LangString::new("N");
    let cases: Vec<(&str, Operation)> = vec![
        (
            r#"{"type":"CreateArtifactPackage","parent":null,"name":{"content":"N"}}"#,
            CreateArtifactPackage::root(name()).into(),
        ),
        (
            r#"{"type":"CreateArtifactPackage","parent":"case","name":{"content":"N"}}"#,
            CreateArtifactPackage::new("case", name()).into(),
        ),
        (
            r#"{"type":"CreateProperty","parent":"p","name":{"content":"N"}}"#,
            CreateProperty::new("p", name()).into(),
        ),
        (
            r#"{"type":"CreateTechnique","parent":"p","name":{"content":"N"}}"#,
            CreateTechnique::new("p", name()).into(),
        ),
        (
            r#"{"type":"CreateParticipant","parent":"p","name":{"content":"N"}}"#,
            CreateParticipant::new("p", name()).into(),
        ),
        (
            r#"{"type":"CreateArtifactGroup","parent":"p","name":{"content":"N"}}"#,
            CreateArtifactGroup::new("p", name()).into(),
        ),
    ];
    for (text, expected) in cases {
        let read: Operation = from_json(text);
        assert_eq!(
            read, expected,
            "{API_006}: {text} does not read as its constructor default"
        );
    }
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

// ---------------------------------------------------------------------------
// Every feature name of an EditError reads back
// ---------------------------------------------------------------------------

/// A document with a Claim `c1` holding a Note `n1`, and an
/// `ArgumentReasoning` `why`, in the ArgumentPackage `p`.
const FEATURE_START: &str = r#"<sacm:ArgumentPackage xmlns:sacm="http://www.omg.org/spec/SACM/20220301" xmlns:xmi="http://www.omg.org/spec/XMI/20131001" xmi:id="p">
  <name content="P"/>
  <argumentElement xmi:type="sacm:Claim" xmi:id="c1">
    <name content="C1"/>
    <note xmi:id="n1"/>
  </argumentElement>
  <argumentElement xmi:type="sacm:ArgumentReasoning" xmi:id="why"><name content="Why"/></argumentElement>
</sacm:ArgumentPackage>
"#;

#[test]
fn lib_api_006_a_feature_not_applicable_reads_back_for_every_feature_name() {
    let doc = match xmi::read(FEATURE_START.as_bytes(), &ReadOptions::default()) {
        Ok(output) => output.into_document(),
        Err(error) => panic!("{API_006}: setup: FEATURE_START does not read: {error}"),
    };
    let cases: [(Operation, ElementKind, &str); 6] = [
        (
            SetName::new("n1", en("N")).into(),
            ElementKind::Note,
            "name",
        ),
        (
            SetAssertionDeclaration::new("why", AssertionDeclaration::Assumed).into(),
            ElementKind::ArgumentReasoning,
            "assertionDeclaration",
        ),
        (
            SetContent::new("c1", two_values()).into(),
            ElementKind::Claim,
            "content",
        ),
        (SetKey::new("n1", en("K")).into(), ElementKind::Note, "key"),
        (
            SetFlag::new("c1", Flag::IsCounter, true).into(),
            ElementKind::Claim,
            "isCounter",
        ),
        (
            AddReference::new("c1", ReferenceEnd::Source, "c1").into(),
            ElementKind::Claim,
            "source",
        ),
    ];
    for (operation, kind, feature) in cases {
        let error = refusal(&doc, operation.clone());
        assert!(
            matches!(
                &error,
                EditError::FeatureNotApplicable { kind: k, feature: f, .. } if *k == kind && *f == feature
            ),
            "{API_006}: setup: {operation:?} is refused as {error:?}, not for its {feature}"
        );
        let written = to_json(&error);
        assert!(
            written.ends_with(&format!(",\"feature\":\"{feature}\"}}")),
            "{API_006}: the feature {feature} is not written as its name: {written}"
        );
        let read: EditError = from_json(&written);
        assert_eq!(
            read, error,
            "{API_006}: a FeatureNotApplicable for {feature} does not read back"
        );
    }
    // No operation of this release refuses a gid, but the name is a feature.
    let gid = r#"{"type":"FeatureNotApplicable","code":"SACM-CMD-008","element":"c1","kind":"Claim","feature":"gid"}"#;
    let read: EditError = from_json(gid);
    assert!(
        matches!(
            &read,
            EditError::FeatureNotApplicable { feature: "gid", kind: ElementKind::Claim, element, .. }
                if element.as_str() == "c1"
        ),
        "{API_006}: {gid} reads as {read:?}"
    );
    assert_eq!(
        to_json(&read),
        gid,
        "{API_006}: a FeatureNotApplicable for gid does not write back"
    );
}

// ---------------------------------------------------------------------------
// R3 and R4 over every object of the golden files
// ---------------------------------------------------------------------------

type Object = serde_json::Map<String, serde_json::Value>;

/// One changed copy of a golden value: a label naming the change, the
/// changed value, and the value it must read as `==` to, or `None` if it
/// must be refused.
struct Changed {
    label: String,
    value: serde_json::Value,
    reads_as: Option<serde_json::Value>,
}

/// What `change` makes of one object: (label, changed object, the object it
/// must read as, or `None`). `root` says whether the object is an element of
/// the file's top-level array.
type Change<'a> = &'a dyn Fn(&Object, bool) -> Vec<(String, Object, Option<Object>)>;

/// Every copy of `value` in which exactly one object, at any depth, is
/// changed by `change`.
fn changed_copies(value: &serde_json::Value, root: bool, change: Change<'_>) -> Vec<Changed> {
    let mut copies = Vec::new();
    match value {
        serde_json::Value::Object(object) => {
            for (label, changed, reads_as) in change(object, root) {
                copies.push(Changed {
                    label,
                    value: serde_json::Value::Object(changed),
                    reads_as: reads_as.map(serde_json::Value::Object),
                });
            }
            for (key, child) in object {
                for inner in changed_copies(child, false, change) {
                    let with = |child: serde_json::Value| {
                        let mut copy = object.clone();
                        copy.insert(key.clone(), child);
                        serde_json::Value::Object(copy)
                    };
                    copies.push(Changed {
                        label: format!("{key}.{}", inner.label),
                        value: with(inner.value),
                        reads_as: inner.reads_as.map(with),
                    });
                }
            }
        }
        serde_json::Value::Array(items) => {
            for (index, item) in items.iter().enumerate() {
                for inner in changed_copies(item, root, change) {
                    let with = |item: serde_json::Value| {
                        let mut copy = items.clone();
                        copy[index] = item;
                        serde_json::Value::Array(copy)
                    };
                    copies.push(Changed {
                        label: format!("[{index}].{}", inner.label),
                        value: with(inner.value),
                        reads_as: inner.reads_as.map(with),
                    });
                }
            }
        }
        _ => {}
    }
    copies
}

/// The golden file `name`, parsed.
fn golden_value(name: &str) -> serde_json::Value {
    match serde_json::from_str(&golden(name)) {
        Ok(value) => value,
        Err(error) => panic!("{API_006}: setup: {name} is not JSON: {error}"),
    }
}

/// The key order of a golden JSON value: each object's keys as written,
/// with the order of the values under them.
enum KeyOrder {
    Object(Vec<(String, KeyOrder)>),
    Array(Vec<KeyOrder>),
    Scalar,
}

impl<'de> serde::Deserialize<'de> for KeyOrder {
    fn deserialize<D: serde::Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        struct Visitor;

        impl<'de> serde::de::Visitor<'de> for Visitor {
            type Value = KeyOrder;

            fn expecting(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
                f.write_str("JSON")
            }

            fn visit_bool<E>(self, _: bool) -> Result<KeyOrder, E> {
                Ok(KeyOrder::Scalar)
            }

            fn visit_i64<E>(self, _: i64) -> Result<KeyOrder, E> {
                Ok(KeyOrder::Scalar)
            }

            fn visit_u64<E>(self, _: u64) -> Result<KeyOrder, E> {
                Ok(KeyOrder::Scalar)
            }

            fn visit_f64<E>(self, _: f64) -> Result<KeyOrder, E> {
                Ok(KeyOrder::Scalar)
            }

            fn visit_str<E>(self, _: &str) -> Result<KeyOrder, E> {
                Ok(KeyOrder::Scalar)
            }

            fn visit_unit<E>(self) -> Result<KeyOrder, E> {
                Ok(KeyOrder::Scalar)
            }

            fn visit_seq<A: serde::de::SeqAccess<'de>>(
                self,
                mut seq: A,
            ) -> Result<KeyOrder, A::Error> {
                let mut items = Vec::new();
                while let Some(item) = seq.next_element()? {
                    items.push(item);
                }
                Ok(KeyOrder::Array(items))
            }

            fn visit_map<A: serde::de::MapAccess<'de>>(
                self,
                mut map: A,
            ) -> Result<KeyOrder, A::Error> {
                let mut keys = Vec::new();
                while let Some(entry) = map.next_entry()? {
                    keys.push(entry);
                }
                Ok(KeyOrder::Object(keys))
            }
        }

        deserializer.deserialize_any(Visitor)
    }
}

/// `value` as compact JSON, each object's keys in the order `order` gives
/// for the object at the same place: the keys it lists, as listed, then any
/// other key, such as one a change added, in `value`'s order.
fn in_golden_order(value: &serde_json::Value, order: &KeyOrder) -> String {
    let none = KeyOrder::Scalar;
    match (value, order) {
        (serde_json::Value::Object(object), _) => {
            let listed: &[(String, KeyOrder)] = match order {
                KeyOrder::Object(keys) => keys,
                _ => &[],
            };
            let mut keys: Vec<(&String, &KeyOrder)> = listed
                .iter()
                .filter(|(key, _)| object.contains_key(key))
                .map(|(key, inner)| (key, inner))
                .collect();
            keys.extend(
                object
                    .keys()
                    .filter(|key| !listed.iter().any(|(listed, _)| listed == *key))
                    .map(|key| (key, &none)),
            );
            let entries: Vec<String> = keys
                .into_iter()
                .map(|(key, inner)| {
                    let child = object.get(key).unwrap_or(&serde_json::Value::Null);
                    format!(
                        "{}:{}",
                        serde_json::Value::String(key.clone()),
                        in_golden_order(child, inner)
                    )
                })
                .collect();
            format!("{{{}}}", entries.join(","))
        }
        (serde_json::Value::Array(items), _) => {
            let orders: &[KeyOrder] = match order {
                KeyOrder::Array(orders) => orders,
                _ => &[],
            };
            let entries: Vec<String> = items
                .iter()
                .enumerate()
                .map(|(index, item)| in_golden_order(item, orders.get(index).unwrap_or(&none)))
                .collect();
            format!("[{}]", entries.join(","))
        }
        (scalar, _) => scalar.to_string(),
    }
}

/// Each copy of the golden file `name` that `change` makes reads as a `T`
/// `==` to its `reads_as`, or is refused as data. Returns the number of
/// copies.
fn assert_changed_copies<T>(name: &str, change: Change<'_>) -> usize
where
    T: DeserializeOwned + PartialEq + Debug,
{
    let copies = changed_copies(&golden_value(name), true, change);
    // `serde_json::Value` sorts an object's keys; each copy is written back
    // with the golden file's key order, which a message's `arguments`
    // object must keep (ADR 0030 amendment 3, R11).
    let order: KeyOrder = from_json(&golden(name));
    for copy in &copies {
        let text = in_golden_order(&copy.value, &order);
        let label = &copy.label;
        match (serde_json::from_str::<T>(&text), &copy.reads_as) {
            (Ok(read), Some(reads_as)) => {
                let expected: T = from_json(&in_golden_order(reads_as, &order));
                assert_eq!(
                    read, expected,
                    "{API_006}: {name} {label}: the value does not read as its default"
                );
            }
            (Ok(read), None) => {
                panic!("{API_006}: {name} {label}: {text} was read, as {read:?}");
            }
            (Err(error), None) => assert!(
                error.is_data(),
                "{API_006}: {name} {label}: refused as malformed JSON, not as data: {error}"
            ),
            (Err(error), Some(_)) => {
                panic!("{API_006}: {name} {label}: {text} was refused: {error}");
            }
        }
    }
    copies.len()
}

/// Runs `assert_changed_copies` over every golden file that reads back, with
/// its type: all but `previews.json`, since a `Preview` is written only.
fn assert_every_golden_changed(change: Change<'_>) {
    let counts = [
        (
            "operations.json",
            assert_changed_copies::<Vec<Operation>>("operations.json", change),
        ),
        (
            "mutations.json",
            assert_changed_copies::<Vec<Mutation>>("mutations.json", change),
        ),
        (
            "edit-errors.json",
            assert_changed_copies::<Vec<EditError>>("edit-errors.json", change),
        ),
        (
            "diagnostics.json",
            assert_changed_copies::<Vec<Diagnostic>>("diagnostics.json", change),
        ),
        (
            "values.json",
            assert_changed_copies::<Values>("values.json", change),
        ),
        (
            "batch-mutations.json",
            assert_changed_copies::<Vec<BatchMutation>>("batch-mutations.json", change),
        ),
        (
            "batch-errors.json",
            assert_changed_copies::<Vec<BatchError>>("batch-errors.json", change),
        ),
        (
            "dangling-references.json",
            assert_changed_copies::<Vec<DanglingReference>>("dangling-references.json", change),
        ),
    ];
    for (name, count) in counts {
        assert!(
            count > 0,
            "{API_006}: setup: no object of {name} was changed"
        );
    }
}

/// The keys of `object` a reader may omit, with the default each reads as
/// (R3): a create's `id` and builder-set keys, a `DeleteElement`'s three
/// policies (the reject variants, ADR 0031), a `LangString`'s `lang` and
/// `content`, and a `Diagnostic`'s `clause`, `affected` and `location`.
fn optional_keys(
    object: &Object,
    root: bool,
    diagnostics: bool,
) -> Vec<(&'static str, serde_json::Value)> {
    use serde_json::Value;
    // A create's `id` is its caller id; the `id` of an AddReference or a
    // RemoveReference is the referenced id, which is required.
    let is_create = object
        .get("type")
        .and_then(Value::as_str)
        .is_some_and(|name| OPERATION_NAMES[..CREATE_COUNT].contains(&name));
    let is_delete = object.get("type").and_then(Value::as_str) == Some("DeleteElement");
    // A MoveElement's `position` may be omitted: after the last (LIB-EDIT-017).
    let is_move = object.get("type").and_then(Value::as_str) == Some("MoveElement");
    if is_move {
        vec![("position", Value::Null)]
    } else if is_delete {
        DELETE_POLICIES
            .iter()
            .map(|(key, default, _)| (*key, Value::String((*default).to_owned())))
            .collect()
    } else if is_create {
        let mut keys = vec![
            ("id", Value::Null),
            ("structure", Value::Null),
            ("reasoning", Value::Null),
            ("is_counter", Value::Bool(false)),
            ("referenced_artifact_elements", Value::Array(Vec::new())),
            ("group_members", Value::Array(Vec::new())),
            ("version", Value::Null),
            ("date", Value::Null),
            ("location", Value::Null),
            ("start_time", Value::Null),
            ("end_time", Value::Null),
        ];
        // The builder-set keys of the creates of slice 0062; `value` is
        // required (ADR 0034 amendment 3).
        keys.extend([
            ("categories", Value::Array(Vec::new())),
            ("origin", Value::Null),
            ("external_reference", Value::Null),
            ("expression_elements", Value::Array(Vec::new())),
        ]);
        // The hrefs of a binding create (LIB-EDIT-020; ADR 0040 amendment 1,
        // A7). An interface's `implements` and `implements_href` are one of
        // two, so neither has a default.
        keys.push(("participant_package_hrefs", Value::Array(Vec::new())));
        keys
    } else if object.get("type").and_then(Value::as_str) == Some("SetReferences") {
        // The hrefs of a set default to none (LIB-EDIT-020).
        vec![("hrefs", Value::Array(Vec::new()))]
    } else if diagnostics && root {
        vec![
            ("clause", Value::Null),
            ("affected", Value::Array(Vec::new())),
            ("location", Value::Null),
        ]
    } else if object.len() == 2 && object.contains_key("lang") && object.contains_key("content") {
        vec![("lang", Value::Null), ("content", Value::Null)]
    } else if object.get("type").and_then(Value::as_str) == Some("ExpressionLangString") {
        // An ExpressionLangString's `lang` and `content` may be left out, as
        // a plain one's; `type` and `expression` are required (ADR 0030
        // amendment 4).
        vec![("lang", Value::Null), ("content", Value::Null)]
    } else {
        Vec::new()
    }
}

#[test]
fn lib_api_006_an_unknown_key_in_any_object_of_the_golden_files_is_refused() {
    let change = |object: &Object, _root: bool| {
        let mut changed = object.clone();
        changed.insert("unknown_key".to_owned(), serde_json::Value::Null);
        vec![("an unknown key".to_owned(), changed, None)]
    };
    assert_every_golden_changed(&change);
}

/// Every copy of `object` without one of its keys: an optional key (see
/// [`optional_keys`]) must read as the object with its default, and every
/// other key, the `"type"` tag included, must be refused.
fn remove_each_key(
    object: &Object,
    root: bool,
    diagnostics: bool,
) -> Vec<(String, Object, Option<Object>)> {
    let optional = optional_keys(object, root, diagnostics);
    object
        .keys()
        .map(|key| {
            let mut without = object.clone();
            without.remove(key);
            let reads_as = optional
                .iter()
                .find(|(name, _)| name == key)
                .map(|(_, default)| {
                    let mut with_default = object.clone();
                    with_default.insert(key.clone(), default.clone());
                    with_default
                });
            (format!("without {key:?}"), without, reads_as)
        })
        .collect()
}

#[test]
fn lib_api_006_each_key_of_every_object_of_the_golden_files_is_required_or_has_its_default() {
    let plain = |object: &Object, root: bool| remove_each_key(object, root, false);
    let diagnostic = |object: &Object, root: bool| remove_each_key(object, root, true);
    let counts = [
        (
            "operations.json",
            assert_changed_copies::<Vec<Operation>>("operations.json", &plain),
        ),
        (
            "mutations.json",
            assert_changed_copies::<Vec<Mutation>>("mutations.json", &plain),
        ),
        (
            "edit-errors.json",
            assert_changed_copies::<Vec<EditError>>("edit-errors.json", &plain),
        ),
        (
            "diagnostics.json",
            assert_changed_copies::<Vec<Diagnostic>>("diagnostics.json", &diagnostic),
        ),
        (
            "values.json",
            assert_changed_copies::<Values>("values.json", &plain),
        ),
        (
            "batch-mutations.json",
            assert_changed_copies::<Vec<BatchMutation>>("batch-mutations.json", &plain),
        ),
        (
            "batch-errors.json",
            assert_changed_copies::<Vec<BatchError>>("batch-errors.json", &plain),
        ),
        (
            "dangling-references.json",
            assert_changed_copies::<Vec<DanglingReference>>("dangling-references.json", &plain),
        ),
    ];
    for (name, count) in counts {
        assert!(count > 0, "{API_006}: setup: no key of {name} was removed");
    }
}

// ---------------------------------------------------------------------------
// The values of slice 0048 (ADR 0031, ADR 0032)
// ---------------------------------------------------------------------------

fn assert_serialize<T: Serialize>() {}

#[test]
fn lib_edit_005_delete_element_implements_serialize_and_deserialize() {
    assert_serde::<DeleteElement>();
}

#[test]
fn lib_edit_004_dangling_reference_reads_and_writes_and_preview_writes() {
    assert_serde::<DanglingReference>();
    assert_serialize::<Preview>();
}

#[test]
fn lib_edit_014_batch_mutation_and_batch_error_implement_serialize_and_deserialize() {
    assert_serde::<BatchMutation>();
    assert_serde::<BatchError>();
}

/// `name` in lower camel case: `reference_policy` is `referencePolicy`.
fn lower_camel(name: &str) -> String {
    let mut parts = name.split('_');
    let mut camel = parts.next().unwrap_or_default().to_owned();
    for part in parts {
        let mut chars = part.chars();
        if let Some(first) = chars.next() {
            camel.extend(first.to_uppercase());
            camel.push_str(chars.as_str());
        }
    }
    camel
}

/// `name` with its first letter in lower case: `RejectIfReferenced` is
/// `rejectIfReferenced`.
fn lower_first(name: &str) -> String {
    let mut chars = name.chars();
    match chars.next() {
        Some(first) => first.to_lowercase().chain(chars).collect(),
        None => String::new(),
    }
}

/// [`DELETE_JSON`] with the policy `key` set to `variant`.
fn delete_json_with(key: &str, variant: &str) -> String {
    let Some((_, default, _)) = DELETE_POLICIES.iter().find(|(name, _, _)| *name == key) else {
        panic!("{EDIT_005}: setup: no policy {key}");
    };
    DELETE_JSON.replacen(
        &format!("\"{key}\":\"{default}\""),
        &format!("\"{key}\":\"{variant}\""),
        1,
    )
}

#[test]
fn lib_edit_005_delete_element_is_written_with_its_element_and_each_policy_by_variant_name() {
    let default = Operation::from(DeleteElement::new("c1"));
    assert_eq!(
        to_json(&default),
        DELETE_JSON,
        "{EDIT_005}: DeleteElement::new is not written with the element and the three reject \
         policies"
    );
    let cases: [(DeleteElement, &str, &str); 4] = [
        (
            DeleteElement::new("c1")
                .with_reference_policy(ReferenceDeletePolicy::DeleteReferencingRelationships),
            "reference_policy",
            "DeleteReferencingRelationships",
        ),
        (
            DeleteElement::new("c1").with_reference_policy(ReferenceDeletePolicy::ScrubReferences),
            "reference_policy",
            "ScrubReferences",
        ),
        (
            DeleteElement::new("c1").with_package_policy(PackageDeletePolicy::DeleteRecursively),
            "package_policy",
            "DeleteRecursively",
        ),
        (
            DeleteElement::new("c1")
                .with_cross_package_policy(CrossPackageReferencePolicy::ApplyReferencePolicy),
            "cross_package_policy",
            "ApplyReferencePolicy",
        ),
    ];
    for (delete, key, variant) in cases {
        let expected = delete_json_with(key, variant);
        let operation = Operation::from(delete.clone());
        assert_eq!(
            to_json(&operation),
            expected,
            "{EDIT_005}: the {key} {variant} is not written as its variant name"
        );
        let read: Operation = from_json(&expected);
        assert_eq!(read, operation, "{EDIT_005}: {expected} does not read back");
        assert_bare_for(EDIT_005, &delete);
    }
    let read: Operation = from_json(DELETE_JSON);
    assert_eq!(
        read, default,
        "{EDIT_005}: {DELETE_JSON} does not read as DeleteElement::new"
    );
}

#[test]
fn lib_edit_005_omitted_delete_policies_read_as_the_reject_defaults() {
    let bare: Operation = from_json(r#"{"type":"DeleteElement","element":"c1"}"#);
    let default = Operation::from(DeleteElement::new("c1"));
    assert_eq!(
        bare, default,
        "{EDIT_005}: a DeleteElement with only its element is not DeleteElement::new"
    );
    for (key, value, _) in DELETE_POLICIES {
        let without = DELETE_JSON.replacen(&format!(",\"{key}\":\"{value}\""), "", 1);
        assert_ne!(
            without, DELETE_JSON,
            "{EDIT_005}: setup: {key} is not in {DELETE_JSON}"
        );
        let read: Operation = from_json(&without);
        assert_eq!(
            read, default,
            "{EDIT_005}: a DeleteElement without {key} does not read as its reject default"
        );
    }
    let scrub: Operation = from_json(
        r#"{"type":"DeleteElement","element":"c1","reference_policy":"ScrubReferences"}"#,
    );
    assert_eq!(
        scrub,
        Operation::from(
            DeleteElement::new("c1").with_reference_policy(ReferenceDeletePolicy::ScrubReferences)
        ),
        "{EDIT_005}: a DeleteElement with one policy does not read the others as the defaults"
    );
}

#[test]
fn lib_edit_005_an_unknown_null_or_misplaced_policy_and_a_missing_element_are_refused() {
    for (key, default, others) in DELETE_POLICIES {
        let value = format!("\"{default}\"");
        let refuse = |what: &str, to: &str| {
            assert_refused_for::<Operation>(EDIT_005, what, DELETE_JSON, &value, to);
        };
        refuse(&format!("an unknown {key}"), "\"Widget\"");
        refuse(
            &format!("a {key} in lower camel case"),
            &format!("\"{}\"", lower_first(default)),
        );
        refuse(&format!("a null {key}"), "null");
        refuse(&format!("a {key} as a number"), "0");
        // A variant of another policy is no variant of this one.
        for (other, other_default, other_variants) in DELETE_POLICIES {
            if other == key {
                continue;
            }
            for variant in other_variants.iter().chain([&other_default]) {
                refuse(
                    &format!("the {other} {variant} as a {key}"),
                    &format!("\"{variant}\""),
                );
            }
        }
        for variant in others {
            let valid = delete_json_with(key, variant);
            assert_refused_for::<Operation>(
                EDIT_005,
                &format!("the {key} {variant} in lower case"),
                &valid,
                variant,
                &variant.to_lowercase(),
            );
        }
        assert_refused_for::<Operation>(
            EDIT_005,
            &format!("a misspelt {key}"),
            DELETE_JSON,
            &format!("\"{key}\""),
            &format!("\"{}\"", lower_camel(key)),
        );
    }
    assert_refused_for::<Operation>(
        EDIT_005,
        "a DeleteElement without element",
        DELETE_JSON,
        r#""element":"c1","#,
        "",
    );
    assert_refused_for::<Operation>(
        EDIT_005,
        "a DeleteElement with a null element",
        DELETE_JSON,
        r#""c1""#,
        "null",
    );
    assert_refused_for::<Operation>(
        EDIT_005,
        "an unknown key in a DeleteElement",
        DELETE_JSON,
        "{",
        r#"{"policy":null,"#,
    );
}

#[test]
fn lib_edit_005_check_and_apply_answer_a_deserialized_delete_as_its_twin() {
    let doc = delete_start();
    let cases: [(&str, DeleteElement); 4] = [
        (
            r#"{"type":"DeleteElement","element":"c1"}"#,
            DeleteElement::new("c1"),
        ),
        (
            r#"{"type":"DeleteElement","element":"r","reference_policy":"ScrubReferences"}"#,
            DeleteElement::new("r").with_reference_policy(ReferenceDeletePolicy::ScrubReferences),
        ),
        (
            r#"{"type":"DeleteElement","element":"c4"}"#,
            DeleteElement::new("c4"),
        ),
        (
            r#"{"type":"DeleteElement","element":"c6","reference_policy":"ScrubReferences","cross_package_policy":"ApplyReferencePolicy"}"#,
            scrubbing_delete("c6"),
        ),
    ];
    for (text, twin) in cases {
        let twin = Operation::from(twin);
        let read: Operation = from_json(text);
        assert_eq!(read, twin, "{EDIT_005}: {text} does not read as its twin");
        assert_eq!(
            edit::check(&doc, &read),
            edit::check(&doc, &twin),
            "{EDIT_005}: check answers {text} otherwise than its twin"
        );
        let (mut via_read, mut via_twin) = (doc.clone(), doc.clone());
        assert_eq!(
            edit::apply(&mut via_read, read),
            edit::apply(&mut via_twin, twin),
            "{EDIT_005}: apply answers {text} otherwise than its twin"
        );
        assert_eq!(
            via_read, via_twin,
            "{EDIT_005}: applying {text} leaves another document than its twin"
        );
    }
}

/// `error` serializes as `expected`, compact, and reads back `==`.
fn assert_error_json(row: &str, error: &EditError, expected: &str) {
    let written = to_json(error);
    assert_eq!(
        written, expected,
        "{row}: {error:?} is not written with its type, code and fields"
    );
    let read: EditError = from_json(&written);
    assert_eq!(&read, error, "{row}: {written} does not read back");
}

#[test]
fn lib_edit_005_a_referenced_refusal_is_written_with_its_code_holder_end_and_id() {
    let error = refusal(&delete_start(), DeleteElement::new("c1"));
    let expected =
        r#"{"type":"Referenced","code":"SACM-CMD-002","holder":"c2","end":"metaClaim","id":"c1"}"#;
    assert_error_json(EDIT_005, &error, expected);
    assert_refused_for::<EditError>(
        EDIT_005,
        "an unknown end in a Referenced",
        expected,
        "metaClaim",
        "metaclaim",
    );
    assert_refused_for::<EditError>(
        EDIT_005,
        "a Referenced with another code",
        expected,
        "SACM-CMD-002",
        "SACM-CMD-006",
    );
}

#[test]
fn lib_edit_006_not_empty_and_cross_package_reference_are_written_with_their_codes() {
    let doc = delete_start();
    let not_empty = r#"{"type":"NotEmpty","code":"SACM-CMD-006","element":"r","contained":1}"#;
    assert_error_json(EDIT_006, &refusal(&doc, DeleteElement::new("r")), not_empty);
    let crossing = r#"{"type":"CrossPackageReference","code":"SACM-CMD-007","holder":"c3","end":"metaClaim","id":"c4"}"#;
    assert_error_json(EDIT_006, &refusal(&doc, DeleteElement::new("c4")), crossing);
    for (what, from, to) in [
        ("a negative count", "1}", "-1}"),
        ("a count as a string", "1}", "\"1\"}"),
        ("a null count", "1}", "null}"),
    ] {
        assert_refused_for::<EditError>(EDIT_006, what, not_empty, from, to);
    }
    assert_refused_for::<EditError>(
        EDIT_006,
        "a CrossPackageReference with the code of Referenced",
        crossing,
        "SACM-CMD-007",
        "SACM-CMD-002",
    );
}

#[test]
fn lib_edit_007_a_real_preview_expired_round_trips_with_its_revisions_as_json_numbers() {
    let mut doc = delete_start();
    let preview = previewed(&doc, SetName::new("c1", en("Renamed")));
    applied(&mut doc, SetName::new("c2", en("Changed first")));
    let error = match edit::apply_preview(&mut doc, &preview) {
        Ok(mutation) => panic!("{EDIT_007}: setup: a stale preview was applied: {mutation:?}"),
        Err(error) => error,
    };
    assert!(
        matches!(
            &error,
            EditError::PreviewExpired { previewed, current, .. }
                if *previewed == preview.revision() && *current == doc.revision()
        ),
        "{EDIT_007}: setup: the stale preview is refused as {error:?}"
    );
    let written = to_json(&error);
    let value: serde_json::Value = from_json(&written);
    let number = |key: &str| value.get(key).and_then(serde_json::Value::as_u64);
    let (Some(previewed), Some(current)) = (number("previewed"), number("current")) else {
        panic!("{EDIT_007}: the revisions of {written} are not JSON numbers");
    };
    assert_eq!(
        written,
        format!(
            r#"{{"type":"PreviewExpired","code":"SACM-CMD-003","previewed":{previewed},"current":{current}}}"#
        ),
        "{EDIT_007}: a PreviewExpired is not written with its type, code and two revisions"
    );
    assert_ne!(
        previewed, current,
        "{EDIT_007}: the two revisions of a stale preview are written as one number"
    );
    let preview_value: serde_json::Value = from_json(&to_json(&preview));
    assert_eq!(
        preview_value
            .get("revision")
            .and_then(serde_json::Value::as_u64),
        Some(previewed),
        "{EDIT_007}: previewed is not written as the preview's revision"
    );
    let read: EditError = from_json(&written);
    assert_eq!(read, error, "{EDIT_007}: {written} does not read back");
    for (what, from, to) in [
        (
            "a revision as a string",
            "\"current\":2",
            "\"current\":\"2\"",
        ),
        ("a null revision", "\"previewed\":1", "\"previewed\":null"),
    ] {
        assert_refused_for::<EditError>(EDIT_007, what, PREVIEW_EXPIRED_JSON, from, to);
    }
}

/// `text` with the number of every `"revision": n` line written as 0.
fn mask_revisions(text: &str) -> String {
    let mut masked = String::new();
    for line in text.lines() {
        let replaced = line
            .split_once("\"revision\": ")
            .and_then(|(indent, rest)| {
                let digits = rest.trim_end_matches(',');
                let comma = &rest[digits.len()..];
                (!digits.is_empty() && digits.bytes().all(|byte| byte.is_ascii_digit()))
                    .then(|| format!("{indent}\"revision\": 0{comma}"))
            });
        masked.push_str(replaced.as_deref().unwrap_or(line));
        masked.push('\n');
    }
    masked
}

/// The JSON number `preview` writes as its revision.
fn revision_number(preview: &Preview) -> u64 {
    let value: serde_json::Value = from_json(&to_json(preview));
    match value.get("revision").and_then(serde_json::Value::as_u64) {
        Some(number) => number,
        None => panic!("{EDIT_004}: the revision of {value} is not a JSON number"),
    }
}

#[test]
fn lib_edit_004_previews_serialize_to_the_golden_json_with_the_revision_as_a_number() {
    let samples = preview_samples();
    let numbers: Vec<u64> = samples.iter().map(revision_number).collect();
    assert!(
        numbers.windows(2).all(|pair| pair[0] == pair[1]),
        "{EDIT_004}: two previews of one document state write different revisions {numbers:?}"
    );
    let mut changed = delete_start();
    applied(&mut changed, SetName::new("c2", en("Changed")));
    let later = previewed(&changed, SetName::new("c1", en("Later")));
    assert_ne!(
        Some(revision_number(&later)),
        numbers.first().copied(),
        "{EDIT_004}: a preview of a changed document writes the same revision"
    );
    for preview in &samples {
        let value: serde_json::Value = from_json(&to_json(preview));
        let keys: BTreeSet<&str> = value
            .as_object()
            .map(|object| object.keys().map(String::as_str).collect())
            .unwrap_or_default();
        assert_eq!(
            keys,
            PREVIEW_KEYS.into_iter().collect::<BTreeSet<_>>(),
            "{EDIT_004}: a Preview is not written with the keys {PREVIEW_KEYS:?}"
        );
        let part = |key: &str| value.get(key).cloned().unwrap_or_default();
        let as_value = |text: String| from_json::<serde_json::Value>(&text);
        assert_eq!(
            part("mutation"),
            as_value(to_json(preview.mutation())),
            "{EDIT_004}: a Preview's mutation is not written as its Mutation"
        );
        assert_eq!(
            part("dangling_references"),
            as_value(to_json(&preview.dangling_references())),
            "{EDIT_004}: a Preview's dangling_references are not written as a list of them"
        );
    }
    let written = match serde_json::to_string_pretty(&samples) {
        Ok(text) => text + "\n",
        Err(error) => panic!("{EDIT_004}: the previews do not serialize: {error}"),
    };
    assert_eq!(
        mask_revisions(&written),
        golden("previews.json"),
        "{EDIT_004}: serializing the previews, revisions written as 0, does not give the bytes \
         of previews.json"
    );
}

#[test]
fn lib_edit_004_dangling_references_serialize_to_the_golden_json_and_read_back_equal() {
    assert_golden_for(
        EDIT_004,
        "dangling-references.json",
        &dangling_reference_samples(),
    );
}

#[test]
fn lib_edit_004_a_dangling_reference_with_an_unknown_name_or_key_is_refused() {
    let valid = r#"{"holder":"g1","end":"argumentElement","id":"c6","crosses_package":false}"#;
    let refuse = |what: &str, from: &str, to: &str| {
        assert_refused_for::<DanglingReference>(EDIT_004, what, valid, from, to);
    };
    refuse("an unknown key", "{", r#"{"position":0,"#);
    refuse("an end by its Rust name", "argumentElement", "GroupMember");
    refuse("an unknown end", "argumentElement", "member");
    refuse("crosses_package as a string", "false", "\"false\"");
    refuse("crosses_package null", "false", "null");
    for key in [
        r#""holder":"g1","#,
        r#""end":"argumentElement","#,
        r#""id":"c6","#,
        r#","crosses_package":false"#,
    ] {
        refuse(&format!("a DanglingReference without {key}"), key, "");
    }
}

#[test]
fn lib_edit_014_batch_mutations_serialize_to_the_golden_json_and_read_back_equal() {
    assert_golden_for(EDIT_014, "batch-mutations.json", &batch_mutation_samples());
}

#[test]
fn lib_edit_014_batch_errors_serialize_to_the_golden_json_and_read_back_equal() {
    assert_golden_for(EDIT_014, "batch-errors.json", &batch_error_samples());
}

#[test]
fn lib_edit_014_a_batch_record_with_an_unknown_key_a_missing_key_or_a_bad_index_is_refused() {
    let batch = r#"{"mutations":[],"created_ids":[],"changed_ids":[],"deleted_ids":[]}"#;
    assert_refused_for::<BatchMutation>(
        EDIT_014,
        "an unknown key in a BatchMutation",
        batch,
        "{",
        r#"{"undone":true,"#,
    );
    for key in [
        r#""mutations":[],"#,
        r#""created_ids":[],"#,
        r#""changed_ids":[],"#,
        r#","deleted_ids":[]"#,
    ] {
        assert_refused_for::<BatchMutation>(
            EDIT_014,
            &format!("a BatchMutation without {key}"),
            batch,
            key,
            "",
        );
    }
    let error = r#"{"index":0,"operation":{"type":"SetName","element":"c1","name":{}},"error":{"type":"ElementNotFound","code":"SACM-CMD-001","element":"c1"}}"#;
    assert_refused_for::<BatchError>(
        EDIT_014,
        "an unknown key in a BatchError",
        error,
        "{",
        r#"{"code":"SACM-CMD-001","#,
    );
    for key in [
        r#""index":0,"#,
        r#""operation":{"type":"SetName","element":"c1","name":{}},"#,
        r#","error":{"type":"ElementNotFound","code":"SACM-CMD-001","element":"c1"}"#,
    ] {
        assert_refused_for::<BatchError>(
            EDIT_014,
            &format!("a BatchError without {key}"),
            error,
            key,
            "",
        );
    }
    for (what, from, to) in [
        ("a negative index", "\"index\":0", "\"index\":-1"),
        ("an index as a string", "\"index\":0", "\"index\":\"0\""),
        ("a null index", "\"index\":0", "\"index\":null"),
        (
            "an error whose code does not match its variant",
            "\"code\":\"SACM-CMD-001\"",
            "\"code\":\"SACM-CMD-004\"",
        ),
    ] {
        assert_refused_for::<BatchError>(EDIT_014, what, error, from, to);
    }
}

#[test]
fn lib_edit_014_a_batch_error_read_back_answers_as_the_one_apply_all_returned() {
    for error in batch_error_samples() {
        let read: BatchError = from_json(&to_json(&error));
        assert_eq!(
            (read.index(), read.operation(), read.error(), read.code()),
            (
                error.index(),
                error.operation(),
                error.error(),
                error.code()
            ),
            "{EDIT_014}: a BatchError read back answers otherwise than the one returned"
        );
    }
}

// ---------------------------------------------------------------------------
// The values of slice 0056 (ADR 0036, ADR 0030 amendment 2)
// ---------------------------------------------------------------------------

/// The three moves of `operations.json`, each with its compact JSON.
const MOVE_JSON: [&str; 3] = [
    r#"{"type":"MoveElement","element":"c1","parent":"evidence","position":null}"#,
    r#"{"type":"MoveElement","element":"c2","parent":"arguments","position":0}"#,
    r#"{"type":"MoveElement","element":"sub","parent":null,"position":1}"#,
];

#[test]
#[expect(
    clippy::too_many_lines,
    reason = "the operation, its refusals and its mutation read best as one list"
)]
fn lib_edit_017_move_element_and_its_refusals_round_trip_as_their_golden_json() {
    assert_serde::<MoveElement>();
    // `new`, `with_position` and `root`.
    let moves = move_samples();
    assert_eq!(
        moves.len(),
        MOVE_JSON.len(),
        "{EDIT_017}: setup: three moves"
    );
    for (operation, expected) in moves.iter().zip(MOVE_JSON) {
        assert_eq!(
            to_json(operation),
            expected,
            "{EDIT_017}: {operation:?} is not written as its golden JSON"
        );
        let read: Operation = from_json(expected);
        assert_eq!(
            &read, operation,
            "{EDIT_017}: {expected} does not read back"
        );
        assert_eq!(
            operation.operation_name(),
            "MoveElement",
            "{EDIT_017}: operation_name()"
        );
    }
    assert_bare_for(EDIT_017, &MoveElement::new("c1", "evidence"));
    assert_bare_for(EDIT_017, &MoveElement::root("sub").with_position(1));

    // `position` may be omitted, as after the last; `parent` may not, even
    // for the roots.
    let omitted: Operation =
        from_json(r#"{"type":"MoveElement","element":"c1","parent":"evidence"}"#);
    assert_eq!(
        omitted,
        Operation::from(MoveElement::new("c1", "evidence")),
        "{EDIT_017}: an omitted position does not read as after the last"
    );
    for (what, valid, from, to) in [
        (
            "a root move without parent",
            MOVE_JSON[2],
            r#""parent":null,"#,
            "",
        ),
        (
            "a move without parent",
            MOVE_JSON[0],
            r#""parent":"evidence","#,
            "",
        ),
        (
            "a move without element",
            MOVE_JSON[0],
            r#""element":"c1","#,
            "",
        ),
        (
            "an unknown key",
            MOVE_JSON[0],
            r#""position":null"#,
            r#""position":null,"after":"c2""#,
        ),
        (
            "a negative position",
            MOVE_JSON[1],
            r#""position":0"#,
            r#""position":-1"#,
        ),
        (
            "a position as a string",
            MOVE_JSON[1],
            r#""position":0"#,
            r#""position":"0""#,
        ),
    ] {
        assert_refused_for::<Operation>(EDIT_017, what, valid, from, to);
    }

    // A deserialized move answers as its twin.
    let doc = error_start();
    let accepted = r#"{"type":"MoveElement","element":"c1","parent":"p","position":1}"#;
    let read: Operation = from_json(accepted);
    assert_eq!(
        read,
        Operation::from(MoveElement::new("c1", "p").with_position(1)),
        "{EDIT_017}: {accepted} does not read as its twin"
    );
    assert_eq!(
        edit::apply(&mut doc.clone(), read),
        edit::apply(
            &mut doc.clone(),
            MoveElement::new("c1", "p").with_position(1)
        ),
        "{EDIT_017}: apply answers {accepted} otherwise than its twin"
    );
    let refused_root = r#"{"type":"MoveElement","element":"c1","parent":null,"position":null}"#;
    let read: Operation = from_json(refused_root);
    let twin = Operation::from(MoveElement::root("c1"));
    assert_eq!(
        read, twin,
        "{EDIT_017}: {refused_root} does not read as its twin"
    );
    let answer = edit::apply(&mut doc.clone(), twin).map(drop);
    assert!(
        matches!(&answer, Err(EditError::InvalidRoot { .. })),
        "{EDIT_017}: setup: the twin of {refused_root} gives {answer:?}"
    );
    assert_eq!(
        edit::check(&doc, &read),
        answer,
        "{EDIT_017}: check answers {refused_root} otherwise than its twin"
    );

    // The three refusals, as `edit-errors.json` holds them.
    let refusals = [
        (
            refusal(&doc, MoveElement::root("c1")),
            r#"{"type":"InvalidRoot","code":"SACM-CMD-005","element":"c1","kind":"Claim"}"#,
        ),
        (
            refusal(&doc, MoveElement::new("p", "p")),
            r#"{"type":"ParentInSubtree","code":"SACM-CMD-005","element":"p","parent":"p"}"#,
        ),
        (
            refusal(&doc, MoveElement::new("c1", "p").with_position(2)),
            r#"{"type":"PositionOutOfRange","code":"SACM-CMD-009","parent":"p","position":2,"children":1}"#,
        ),
        (
            refusal(&doc, MoveElement::root("case").with_position(1)),
            r#"{"type":"PositionOutOfRange","code":"SACM-CMD-009","parent":null,"position":1,"children":0}"#,
        ),
    ];
    for (error, expected) in &refusals {
        assert_error_json(EDIT_017, error, expected);
    }
    for (what, valid, from, to) in [
        (
            "an InvalidRoot with the code of another refusal",
            refusals[0].1,
            "SACM-CMD-005",
            "SACM-CMD-009",
        ),
        (
            "a PositionOutOfRange without parent",
            refusals[3].1,
            r#""parent":null,"#,
            "",
        ),
        (
            "a ParentInSubtree with an unknown key",
            refusals[1].1,
            r#""parent":"p""#,
            r#""parent":"p","depth":1"#,
        ),
    ] {
        assert_refused_for::<EditError>(EDIT_017, what, valid, from, to);
    }

    // The move of `mutations.json`.
    let Some(move_mutation) = mutation_samples().pop() else {
        panic!("{EDIT_017}: setup: no mutation sample");
    };
    assert_eq!(
        to_json(&move_mutation),
        r#"{"operation":{"type":"MoveElement","element":"c2","parent":"q","position":null},"created_ids":[],"changed_ids":["c2"],"deleted_ids":[]}"#,
        "{EDIT_017}: the move's Mutation is not written as its golden JSON"
    );
    let read: Mutation = from_json(&to_json(&move_mutation));
    assert_eq!(
        read, move_mutation,
        "{EDIT_017}: the move's Mutation does not read back"
    );

    // The golden files hold them: `operations.json` the three moves right
    // after the ReorderElements samples, `edit-errors.json` the four
    // refusals just before the slice-0076 sample, its last, and
    // `mutations.json` the move last. (The bytes are
    // fixed by the LIB-API-006 golden tests, over every sample.)
    let value = |text: &str| -> serde_json::Value { from_json(text) };
    let items = |name: &str| -> Vec<serde_json::Value> {
        match golden_value(name) {
            serde_json::Value::Array(items) => items,
            other => panic!("{EDIT_017}: setup: {name} is no array: {other}"),
        }
    };
    let operations = items("operations.json");
    let Some(first) = operations
        .iter()
        .position(|item| item.get("type") == Some(&value(r#""MoveElement""#)))
    else {
        panic!("{EDIT_017}: operations.json holds no MoveElement sample");
    };
    let expected: Vec<serde_json::Value> = MOVE_JSON.iter().map(|text| value(text)).collect();
    assert_eq!(
        operations.get(first..first + 3),
        Some(expected.as_slice()),
        "{EDIT_017}: operations.json does not hold the three moves in order"
    );
    assert_eq!(
        (
            first
                .checked_sub(1)
                .and_then(|index| operations.get(index))
                .and_then(|item| item.get("type")),
            operations.get(first + 3).and_then(|item| item.get("type")),
        ),
        (
            Some(&value(r#""ReorderElements""#)),
            Some(&value(r#""DeleteElement""#))
        ),
        "{EDIT_017}: the moves of operations.json are not after the ReorderElements samples"
    );
    let errors = items("edit-errors.json");
    let expected: Vec<serde_json::Value> = refusals.iter().map(|(_, text)| value(text)).collect();
    assert_eq!(
        errors.get(errors.len().saturating_sub(5)..errors.len().saturating_sub(1)),
        Some(expected.as_slice()),
        "{EDIT_017}: edit-errors.json does not hold the four refusals just before the slice-0076 sample"
    );
    assert_eq!(
        items("mutations.json").last(),
        Some(&value(&to_json(&move_mutation))),
        "{EDIT_017}: mutations.json does not end with the move"
    );
}

#[test]
fn lib_edit_018_copy_elements_is_neither_serialized_nor_deserialized() {
    let source = error_start();
    let copy = match CopyElements::new(&source, ["c1"], "p") {
        Ok(copy) => copy,
        Err(error) => panic!("{EDIT_018}: setup: the copy was refused: {error}"),
    };
    let operation = Operation::from(copy.clone());
    assert_eq!(
        operation.operation_name(),
        "CopyElements",
        "{EDIT_018}: operation_name()"
    );

    // Serializing the operation, or anything that holds it, is an Err.
    assert!(
        serde_json::to_string(&operation).is_err(),
        "{EDIT_018}: Operation::CopyElements serializes"
    );
    let mut target = error_start();
    let preview = match edit::preview(&target, &operation) {
        Ok(preview) => preview,
        Err(error) => panic!("{EDIT_018}: setup: the copy was refused: {error}"),
    };
    assert!(
        serde_json::to_string(preview.mutation()).is_err(),
        "{EDIT_018}: a Mutation holding a copy serializes"
    );
    assert!(
        serde_json::to_string(&preview).is_err(),
        "{EDIT_018}: a Preview holding a copy serializes"
    );
    let mutation = applied(&mut target, copy.clone());
    assert!(
        serde_json::to_string(&mutation).is_err(),
        "{EDIT_018}: an applied copy's Mutation serializes"
    );
    assert!(
        serde_json::to_string_pretty(&vec![mutation]).is_err(),
        "{EDIT_018}: a list of Mutations holding a copy serializes"
    );
    let batch = committed(edit::apply_all(&mut error_start(), [copy]));
    assert!(
        serde_json::to_string(&batch).is_err(),
        "{EDIT_018}: a BatchMutation holding a copy serializes"
    );
    // Control: the same values without a copy serialize.
    assert!(
        serde_json::to_string(&committed(edit::apply_all(
            &mut error_start(),
            [MoveElement::new("c1", "p")]
        )))
        .is_ok(),
        "{EDIT_018}: setup: a BatchMutation without a copy does not serialize"
    );

    // The tag does not deserialize, with or without fields.
    for text in [
        r#"{"type":"CopyElements"}"#,
        r#"{"type":"CopyElements","parent":"p"}"#,
        r#"{"type":"CopyElements","parent":"p","source_ids":["c1"]}"#,
        r#"{"type":"CopyElements","parent":null,"copied":{}}"#,
    ] {
        match serde_json::from_str::<Operation>(text) {
            Ok(read) => panic!("{EDIT_018}: {text} was read as {read:?}"),
            Err(error) => assert!(
                error.is_data(),
                "{EDIT_018}: {text} is refused as malformed JSON, not as data: {error}"
            ),
        }
    }
    let in_a_mutation = r#"{"operation":{"type":"CopyElements"},"created_ids":[],"changed_ids":[],"deleted_ids":[]}"#;
    assert!(
        serde_json::from_str::<Mutation>(in_a_mutation).is_err(),
        "{EDIT_018}: a Mutation holding the tag CopyElements reads"
    );
    // Control: a valid twin of the tag reads.
    let _: Operation = from_json(MOVE_JSON[0]);
}

// ---------------------------------------------------------------------------
// SACM23-ART-010: the ends and the create of slice 0057
// ---------------------------------------------------------------------------

#[test]
fn sacm23_art_010_the_two_ends_round_trip_as_artifact_source_and_artifact_target() {
    for (end, serde_name) in [
        (ReferenceEnd::ArtifactSource, "artifactSource"),
        (ReferenceEnd::ArtifactTarget, "artifactTarget"),
    ] {
        let written = to_json(&end);
        assert_eq!(
            written,
            format!("\"{serde_name}\""),
            "{ART_010}: {end:?} is not serialized as {serde_name}"
        );
        let read: ReferenceEnd = from_json(&written);
        assert_eq!(read, end, "{ART_010}: {serde_name} does not read back");
    }
    // The XMI names stay the AssertedRelationship's ends.
    let source: ReferenceEnd = from_json("\"source\"");
    let target: ReferenceEnd = from_json("\"target\"");
    assert_eq!(
        (source, target),
        (ReferenceEnd::Source, ReferenceEnd::Target),
        "{ART_010}: \"source\" and \"target\" do not read as an AssertedRelationship's ends"
    );
    let set =
        r#"{"type":"SetReferences","element":"rel1","end":"artifactTarget","ids":["testing"]}"#;
    let read: Operation = from_json(set);
    assert_eq!(
        read,
        Operation::from(SetReferences::new(
            "rel1",
            ReferenceEnd::ArtifactTarget,
            ["testing"]
        )),
        "{ART_010}: {set} does not read as its twin"
    );
    assert_refused_for::<Operation>(
        ART_010,
        "the end by its Rust name",
        set,
        "artifactTarget",
        "ArtifactTarget",
    );
}

#[test]
fn sacm23_art_010_a_create_artifact_asset_relationship_reads_with_its_default_id_and_requires_its_ends()
 {
    let minimal = r#"{"type":"CreateArtifactAssetRelationship","parent":"evidence","name":{"content":"N"},"sources":["report"],"targets":["testing"]}"#;
    let read: Operation = from_json(minimal);
    assert_eq!(
        read,
        Operation::from(CreateArtifactAssetRelationship::new(
            "evidence",
            LangString::new("N"),
            ["report"],
            ["testing"]
        )),
        "{ART_010}: {minimal} does not read as its constructor default"
    );
    for key in ["\"sources\":[\"report\"],", ",\"targets\":[\"testing\"]"] {
        assert_refused_for::<Operation>(
            ART_010,
            &format!("a create without {key}"),
            minimal,
            key,
            "",
        );
    }
    assert_refused_for::<Operation>(
        ART_010,
        "a create with an AssertedRelationship's target",
        minimal,
        "\"targets\":[\"testing\"]",
        "\"target\":\"testing\"",
    );
}

// ---------------------------------------------------------------------------
// Slice 0062: the Terminology operations (ADR 0034 amendment 3)
// ---------------------------------------------------------------------------

/// `end` is written as `name`, `name` reads as `end`, and a `SetReferences`
/// naming it reads as its twin.
fn assert_end_json(row: &str, end: ReferenceEnd, name: &str) {
    let written = to_json(&end);
    assert_eq!(
        written,
        format!("\"{name}\""),
        "{row}: {end:?} is not serialized as {name}"
    );
    let read: ReferenceEnd = from_json(&written);
    assert_eq!(read, end, "{row}: {name} does not read back");
    let set = format!(r#"{{"type":"SetReferences","element":"x","end":"{name}","ids":["y"]}}"#);
    let read: Operation = from_json(&set);
    assert_eq!(
        read,
        Operation::from(SetReferences::new("x", end, ["y"])),
        "{row}: {set} does not read as its twin"
    );
}

#[test]
fn sacm23_term_002_the_category_end_round_trips_as_category() {
    assert_end_json("SACM23-TERM-002", ReferenceEnd::Category, "category");
}

#[test]
fn sacm23_term_004_the_origin_end_round_trips_as_origin() {
    assert_end_json(TERM_004, ReferenceEnd::Origin, "origin");
}

#[test]
fn sacm23_term_005_the_element_end_round_trips_as_element() {
    assert_end_json("SACM23-TERM-005", ReferenceEnd::Element, "element");
}

#[test]
fn sacm23_term_006_the_member_end_round_trips_as_terminology_element() {
    assert_end_json(
        TERM_006,
        ReferenceEnd::TerminologyGroupMember,
        "terminologyElement",
    );
    // The Rust name is no serde name.
    assert_refused_for::<ReferenceEnd>(
        TERM_006,
        "the end by its Rust name",
        "\"terminologyElement\"",
        "terminologyElement",
        "TerminologyGroupMember",
    );
}

#[test]
fn sacm23_term_001_a_create_terminology_package_without_parent_is_refused() {
    let root = r#"{"type":"CreateTerminologyPackage","parent":null,"name":{"content":"N"}}"#;
    let read: Operation = from_json(root);
    assert_eq!(
        read,
        Operation::from(CreateTerminologyPackage::root(LangString::new("N"))),
        "{TERM_001}: {root} does not read as root(name)"
    );
    assert_refused_for::<Operation>(
        TERM_001,
        "a TerminologyPackage create without parent",
        root,
        r#""parent":null,"#,
        "",
    );
}

#[test]
fn sacm23_term_004_a_create_term_reads_with_its_defaults_and_requires_its_value() {
    let minimal = r#"{"type":"CreateTerm","parent":"terms","name":{"content":"N"},"value":"v"}"#;
    let read: Operation = from_json(minimal);
    assert_eq!(
        read,
        Operation::from(CreateTerm::new("terms", LangString::new("N"), "v")),
        "{TERM_004}: {minimal} does not read as its constructor default"
    );
    assert_refused_for::<Operation>(
        TERM_004,
        "a CreateTerm without value",
        minimal,
        r#","value":"v""#,
        "",
    );
    let expression =
        r#"{"type":"CreateExpression","parent":"terms","name":{"content":"N"},"value":"v"}"#;
    let read: Operation = from_json(expression);
    assert_eq!(
        read,
        Operation::from(CreateExpression::new("terms", LangString::new("N"), "v")),
        "{TERM_004}: {expression} does not read as its constructor default"
    );
    assert_refused_for::<Operation>(
        TERM_004,
        "a CreateExpression without value",
        expression,
        r#","value":"v""#,
        "",
    );
    // SetValue has no removal: a null value is refused.
    let set = r#"{"type":"SetValue","element":"t1","value":"v"}"#;
    let read: Operation = from_json(set);
    assert_eq!(
        read,
        Operation::from(SetValue::new("t1", "v")),
        "{TERM_004}: {set} does not read as its twin"
    );
    assert_refused_for::<Operation>(TERM_004, "a null value", set, r#""v""#, "null");
}

// ---------------------------------------------------------------------------
// The creates of slice 0066 (SACM23-ARG-014, -015, SACM23-ART-012, -013)
// ---------------------------------------------------------------------------

/// Each of `samples` (a constructed create and its compact JSON) is written
/// as that JSON, reads back as its twin, is in `operations.json`, and its
/// bare struct is the operation without `"type"`; and the JSON without
/// `required` (a key and its value, with the comma before it) is refused.
fn assert_create_json<T>(row: &str, samples: &[(T, &str)], required: &str)
where
    T: Serialize + DeserializeOwned + Into<Operation> + Clone + PartialEq + Debug,
{
    let in_golden: Vec<Operation> = from_json(&golden("operations.json"));
    for (create, json) in samples {
        let operation: Operation = create.clone().into();
        assert_eq!(
            to_json(&operation),
            *json,
            "{row}: {operation:?} is not written as {json}"
        );
        let read: Operation = from_json(json);
        assert_eq!(read, operation, "{row}: {json} does not read as its twin");
        assert!(
            in_golden.contains(&operation),
            "{row}: operations.json does not hold {json}"
        );
        assert_bare_for(row, create);
        assert_refused_for::<Operation>(row, "the end's key left out", json, required, "");
        assert_refused_for::<Operation>(
            row,
            "an unknown key",
            json,
            r#""name":"#,
            r#""unknown":1,"name":"#,
        );
    }
    // The parent key is required: null is a root.
    let (_, json) = &samples[0];
    assert_refused_for::<Operation>(
        row,
        "the parent key left out",
        json,
        r#""parent":null,"#,
        "",
    );
}

#[test]
fn sacm23_arg_014_create_interface_serializes_to_the_golden_json() {
    let name = || en("Braking interface");
    assert_create_json(
        ARG_014,
        &[
            (
                CreateArgumentPackageInterface::root(name(), "braking").with_id("braking_api"),
                r#"{"type":"CreateArgumentPackageInterface","parent":null,"id":"braking_api","name":{"lang":"en","content":"Braking interface"},"implements":"braking"}"#,
            ),
            (
                CreateArgumentPackageInterface::new("braking", name(), "braking"),
                r#"{"type":"CreateArgumentPackageInterface","parent":"braking","id":null,"name":{"lang":"en","content":"Braking interface"},"implements":"braking"}"#,
            ),
        ],
        r#","implements":"braking""#,
    );
    // The id may be left out; it reads as a generated one.
    let read: Operation = from_json(
        r#"{"type":"CreateArgumentPackageInterface","parent":"braking","name":{"lang":"en","content":"Braking interface"},"implements":"braking"}"#,
    );
    assert_eq!(
        read,
        Operation::from(CreateArgumentPackageInterface::new(
            "braking",
            name(),
            "braking"
        )),
        "{ARG_014}: a create without id does not read as new()"
    );
}

#[test]
fn sacm23_arg_015_create_binding_serializes_to_the_golden_json() {
    let name = || en("Braking supports the vehicle");
    let participants = ["braking_api", "vehicle_api"];
    assert_create_json(
        ARG_015,
        &[
            (
                CreateArgumentPackageBinding::root(name(), participants).with_id("binding"),
                r#"{"type":"CreateArgumentPackageBinding","parent":null,"id":"binding","name":{"lang":"en","content":"Braking supports the vehicle"},"participant_packages":["braking_api","vehicle_api"]}"#,
            ),
            (
                CreateArgumentPackageBinding::new("case", name(), participants),
                r#"{"type":"CreateArgumentPackageBinding","parent":"case","id":null,"name":{"lang":"en","content":"Braking supports the vehicle"},"participant_packages":["braking_api","vehicle_api"]}"#,
            ),
        ],
        r#","participant_packages":["braking_api","vehicle_api"]"#,
    );
    let add = r#"{"type":"AddReference","element":"binding","end":"participantPackage","id":"vehicle_api"}"#;
    assert_eq!(
        from_json::<Operation>(add),
        Operation::from(AddReference::new(
            "binding",
            ReferenceEnd::ParticipantPackage,
            "vehicle_api"
        )),
        "{ARG_015}: {add} does not read as its twin"
    );
    let errors: Vec<EditError> = from_json(&golden("edit-errors.json"));
    assert!(
        errors.iter().any(|error| matches!(
            error,
            EditError::TooFewReferences {
                end: "participantPackage",
                given: 1,
                required: 2,
                ..
            }
        )),
        "{ARG_015}: edit-errors.json holds no TooFewReferences of participantPackage"
    );
}

#[test]
fn sacm23_art_012_create_interface_serializes_to_the_golden_json() {
    let name = || en("Brake tests interface");
    assert_create_json(
        ART_012,
        &[
            (
                CreateArtifactPackageInterface::root(name(), "tests").with_id("tests_api"),
                r#"{"type":"CreateArtifactPackageInterface","parent":null,"id":"tests_api","name":{"lang":"en","content":"Brake tests interface"},"implements":"tests"}"#,
            ),
            (
                CreateArtifactPackageInterface::new("tests", name(), "tests"),
                r#"{"type":"CreateArtifactPackageInterface","parent":"tests","id":null,"name":{"lang":"en","content":"Brake tests interface"},"implements":"tests"}"#,
            ),
        ],
        r#","implements":"tests""#,
    );
}

#[test]
fn sacm23_art_013_create_binding_serializes_to_the_golden_json() {
    let name = || en("The tests verify the requirement");
    let participants = ["tests_api", "requirements_api"];
    assert_create_json(
        ART_013,
        &[
            (
                CreateArtifactPackageBinding::root(name(), participants).with_id("binding"),
                r#"{"type":"CreateArtifactPackageBinding","parent":null,"id":"binding","name":{"lang":"en","content":"The tests verify the requirement"},"participant_packages":["tests_api","requirements_api"]}"#,
            ),
            (
                CreateArtifactPackageBinding::new("case", name(), participants),
                r#"{"type":"CreateArtifactPackageBinding","parent":"case","id":null,"name":{"lang":"en","content":"The tests verify the requirement"},"participant_packages":["tests_api","requirements_api"]}"#,
            ),
        ],
        r#","participant_packages":["tests_api","requirements_api"]"#,
    );
}

#[test]
fn sacm23_ac_002_create_interface_serializes_to_the_golden_json() {
    assert_serde::<CreateAssuranceCasePackageInterface>();
    let name = || en("Braking case interface");
    assert_create_json(
        AC_002,
        &[
            (
                CreateAssuranceCasePackageInterface::root(name(), "braking_case")
                    .with_id("braking_case_api"),
                r#"{"type":"CreateAssuranceCasePackageInterface","parent":null,"id":"braking_case_api","name":{"lang":"en","content":"Braking case interface"},"implements":"braking_case"}"#,
            ),
            (
                CreateAssuranceCasePackageInterface::new("braking_case", name(), "braking_case"),
                r#"{"type":"CreateAssuranceCasePackageInterface","parent":"braking_case","id":null,"name":{"lang":"en","content":"Braking case interface"},"implements":"braking_case"}"#,
            ),
        ],
        r#","implements":"braking_case""#,
    );
}

#[test]
fn sacm23_ac_003_create_binding_serializes_to_the_golden_json() {
    assert_serde::<CreateAssuranceCasePackageBinding>();
    let name = || en("The braking case supports the vehicle case");
    let participants = ["braking_case_api", "vehicle_case_api"];
    assert_create_json(
        AC_003,
        &[
            (
                CreateAssuranceCasePackageBinding::root(name(), participants).with_id("binding"),
                r#"{"type":"CreateAssuranceCasePackageBinding","parent":null,"id":"binding","name":{"lang":"en","content":"The braking case supports the vehicle case"},"participant_packages":["braking_case_api","vehicle_case_api"]}"#,
            ),
            (
                CreateAssuranceCasePackageBinding::new("system", name(), participants),
                r#"{"type":"CreateAssuranceCasePackageBinding","parent":"system","id":null,"name":{"lang":"en","content":"The braking case supports the vehicle case"},"participant_packages":["braking_case_api","vehicle_case_api"]}"#,
            ),
        ],
        r#","participant_packages":["braking_case_api","vehicle_case_api"]"#,
    );
}

#[test]
fn sacm23_term_007_create_interface_serializes_to_the_golden_json() {
    assert_serde::<CreateTerminologyPackageInterface>();
    let name = || en("Hazard terminology interface");
    assert_create_json(
        TERM_007,
        &[
            (
                CreateTerminologyPackageInterface::root(name(), "hazard_terms")
                    .with_id("hazard_terms_api"),
                r#"{"type":"CreateTerminologyPackageInterface","parent":null,"id":"hazard_terms_api","name":{"lang":"en","content":"Hazard terminology interface"},"implements":"hazard_terms"}"#,
            ),
            (
                CreateTerminologyPackageInterface::new("hazard_terms", name(), "hazard_terms"),
                r#"{"type":"CreateTerminologyPackageInterface","parent":"hazard_terms","id":null,"name":{"lang":"en","content":"Hazard terminology interface"},"implements":"hazard_terms"}"#,
            ),
        ],
        r#","implements":"hazard_terms""#,
    );
}

#[test]
fn sacm23_term_008_create_binding_serializes_to_the_golden_json() {
    assert_serde::<CreateTerminologyPackageBinding>();
    let name = || en("Hazard terms map to design terms");
    let participants = ["hazard_terms_api", "design_terms_api"];
    assert_create_json(
        TERM_008,
        &[
            (
                CreateTerminologyPackageBinding::root(name(), participants).with_id("binding"),
                r#"{"type":"CreateTerminologyPackageBinding","parent":null,"id":"binding","name":{"lang":"en","content":"Hazard terms map to design terms"},"participant_packages":["hazard_terms_api","design_terms_api"]}"#,
            ),
            (
                CreateTerminologyPackageBinding::new("case", name(), participants),
                r#"{"type":"CreateTerminologyPackageBinding","parent":"case","id":null,"name":{"lang":"en","content":"Hazard terms map to design terms"},"participant_packages":["hazard_terms_api","design_terms_api"]}"#,
            ),
        ],
        r#","participant_packages":["hazard_terms_api","design_terms_api"]"#,
    );
}
