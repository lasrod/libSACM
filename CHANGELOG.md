# Changelog

All notable changes to this project are documented here. The format follows
[Keep a Changelog](https://keepachangelog.com/en/1.1.0/), and the project adheres to
[Semantic Versioning](https://semver.org/spec/v2.0.0.html). Before 1.0, a breaking
change bumps the minor version and is listed under **Breaking**. A strict `xmi::write`
checks schema validity only, so a new validation rule never changes what it writes. A new
strict read refusal, or a new strict write refusal, is listed under **Changed**.

## [Unreleased]

### Added

- Cargo workspace: the `sacm` library, `sacm-cli` (`sacm version`) and `xtask`.
- `sacm::VERSION` and `sacm::SACM_VERSION`.
- The functionality matrix and its gate (`cargo xtask matrix check|render|next|status`):
  every row's status must match its tests and verification record.
- Hash-pinned download of the OMG SACM 2.3 references (`cargo xtask fetch-spec`).
- `cargo xtask fetch-spec` also fetches XMI 2.5.1 (formal/15-06-07), hash-pinned
  [LIB-TOOL-020].
- CI gates: rustfmt, clippy, tests on Linux, Windows and macOS, MSRV 1.85, rustdoc,
  cargo-deny, the matrix gate and the agent-hook tests. Third-party actions are pinned
  by commit SHA.
- Mutation testing with cargo-mutants under cargo-nextest: a PR job mutates the diff,
  and a weekly sharded run covers the workspace. Survivors are triaged in
  `docs/quality/mutants-triage.md`.
- Agent roles, skills and a write-scope hook for the engineering loop.
- `sacm::model`: the read-only document model (`Document`, `ElementId` and the
  `SacmElement` view, with roots, look-up by id, parent, children and pre-order
  traversal) and the metamodel's element kinds (`ElementKind`, `AbstractClass`,
  `MetamodelPackage`) [LIB-API-002, LIB-API-003].
- The `test-support` feature, which makes `Document::insert` and the diagnostic
  constructors public for tests. It is not part of the stable API.
- `sacm::diagnostic`: `Diagnostic` (code, severity, matrix row, clause, affected ids,
  source location, message), `Severity`, `Code` and `SourceLocation`, with a one-line
  `Display` and one total order [LIB-DIAG-001].
- Error convention (ADR 0011): one `#[non_exhaustive]` error type per operation (an enum,
  or a struct with private fields, ADR 0011 amendment 2).
  `InsertError` messages quote and escape ids [LIB-API-004].
- `sacm::model::LangString`: an optional language tag and optional content, with
  absent, empty and non-empty kept distinct (ADR 0012) [SACM23-BASE-001].
- `sacm::model::AssertionDeclaration`: the six SACM 2.3 literals, with `asserted` as the
  default and exact `as_str`/`FromStr`; a failed parse is a
  `ParseAssertionDeclarationError` [SACM23-ARG-003].
- `sacm::model::MultiLangString`: an ordered list of `LangString` values; duplicates are
  kept and equality depends on order [SACM23-BASE-002].
- `SacmElement::name`: an element's `name`, or `None` when it is missing; nothing invents
  a name (ADR 0013) [SACM23-BASE-003].
- The unstable `test-support` `Document::insert` takes a name, and refuses one for the
  four UtilityElement kinds with `InsertError::NameNotAllowed` [SACM23-BASE-003].
- The unstable `test-support` `Document::insert` refuses a child that no SACM 2.3
  containment role of its parent admits, with `InsertError::ContainmentNotAllowed`. An
  AssuranceCasePackage holds sub-packages through its four roles, an ArtifactPackage
  admits every ArtifactElement (ADR 0014). A root that is not a package kind is
  refused with `InsertError::RootNotAllowed` [SACM23-AC-001, LIB-API-008].
- `SacmElement::assertion_declaration`: an Assertion's `assertionDeclaration` (`asserted`
  unless the document holds another), and `None` for every other kind. The unstable
  `test-support` `Document::set_assertion_declaration` sets it, refusing an unknown id or
  a non-Assertion with `SetAssertionDeclarationError` [SACM23-ARG-002].
- ArgumentPackage as a root, or nested in an AssuranceCasePackage, an ArtifactPackage or
  another ArgumentPackage, holding argumentation elements in document order; the §11.4
  homogeneity rule is left to validation [SACM23-ARG-001].
- `sacm::xmi`: `read(&[u8], &ReadOptions)`, `ReadOptions`, `ReadOutput` and `ReadError`
  (ADR 0015). `read` refuses a DOCTYPE (`ReadError::Doctype`, SACM-SEC-001), malformed
  or truncated input and invalid UTF-8 (`ReadError::Malformed`, SACM-XML-001), and
  anything it cannot represent yet (`ReadError::Unsupported`); it never returns a
  partial document. XML is parsed with `roxmltree`, privately (ADR 0005)
  [LIB-SEC-001, LIB-SEC-002, LIB-API-009].
- `Code::XML_MALFORMED` and `Code::XML_DOCTYPE_REJECTED`, the first catalog codes
  [LIB-SEC-001, LIB-SEC-002].
- `xmi::read` reads AssuranceCasePackage and ArgumentPackage roots, bare or in an
  `xmi:XMI` element, into a `Document` (`ReadOutput::document`, `diagnostics` and
  `into_document`). A root that is not a package kind is `ReadError::InvalidRoot`, and an
  `xmi:XMI` element with no package is `ReadError::NoPackage`, both
  `Code::XMI_INVALID_ROOT` (SACM-XMI-001). Content it cannot read yet stays
  `Unsupported` (ADR 0016) [SACM23-XMI-001].
- `xmi::read` refuses XML the parser let through as `Malformed`: a bad XML declaration,
  a character reference to a surrogate or above U+10FFFF, `xmlns:p=""`, an element or
  attribute name with an empty prefix (`<:c>`), and a processing-instruction target that
  is `xml`, contains a colon, or is followed by neither white space nor `?>`
  [LIB-SEC-005].
- `xmi::read` refuses a declared `xmlns:xmlns` prefix as `Malformed` (Namespaces 1.0
  §3). `cargo xtask fetch-spec` also fetches the W3C XML Conformance Test Suite
  20130923 (hash-pinned, fetch-only, never committed) into `third_party/xmlconf/`;
  `--only xmlconf` fetches only the suite. Every not-well-formed XML 1.0 and Namespaces
  1.0 case in the suite is refused [LIB-SEC-006].
- `xmi::read` does not depend on prefixes. Names are matched by namespace URI, and an
  `xmi:type` or `xsi:type` value is resolved against the namespaces in scope, after XML
  white space is trimmed from its ends. A root under `xmlns=""` is reported as in no
  namespace [SACM23-XMI-002].
- `xmi::read` reads UTF-16 input that starts with a byte order mark. A declared encoding
  that contradicts the mark, an odd byte count and an unpaired surrogate are `Malformed`.
  UTF-16 without a byte order mark, UTF-32 and UCS-4 stay `Unsupported`
  [SACM23-XMI-018].
- `sacm::xmi`: `write(&Document, &WriteOptions) -> Result<String, WriteError>`,
  `WriteOptions` and `WriteError` (ADR 0017). Output is deterministic: UTF-8, LF, two
  spaces per depth, the pinned `sacm` and `xmi` namespaces on the document element only,
  one root bare or several in `xmi:XMI`, children under their
  role names in document order, `xmi:type` where the role does not give the class, and
  an `xmi:id` on every element. A document with no root is `WriteError::NoRoot`, and an
  id or name that XML 1.0 cannot represent is `WriteError::UnrepresentableCharacter`
  [SACM23-XMI-003, SACM23-XMI-004, SACM23-XMI-006, SACM23-XMI-007, SACM23-XMI-013,
  LIB-API-010].
- Names in XMI (ADR 0017 amendment 1). `xmi::write` writes an element's name as its first
  child, `<name lang="…" content="…" />`, keeping absent and empty fields distinct.
  `xmi::read` reads one `name` child of a package root, with its values exactly as
  XML 1.0 gives them (never trimmed); anything else on or in `name` stays `Unsupported`,
  except what later entries read.
  `WriteError::UnrepresentableCharacter` names the attribute that holds the character
  (`WriteAttribute`) [SACM23-XMI-011, SACM23-XMI-012, LIB-API-010].
- `sacm::validation::validate(&Document) -> Vec<Diagnostic>` (ADR 0018): reports, sorted
  and without changing the document, each ModelElement with no name as an Error with
  `Code::MULTIPLICITY_VIOLATION` (SACM-MULT-001, clause 8.6). Reading still loads a
  nameless document. `diagnostic::has_errors` says whether a list holds an Error
  [SACM23-VAL-018].
- `sacm::edit`, the first editing API (ADR 0019): `apply(&mut Document, impl
  Into<Operation>) -> Result<Mutation, EditError>` with `CreateAssuranceCasePackage`,
  `CreateArgumentPackage` (each a root or under a parent) and `CreateClaim`, each with a
  required `LangString` name and an optional `with_id`. A parent is legal when SACM lets
  its class contain the new kind (the same rules `xmi::read` uses). A caller id must be
  an XML NCName; otherwise the id is generated as `<snake_case kind>_<n>`,
  deterministically. `Mutation::created_ids`
  lists what was created. Refusals, in check order, leave the document unchanged:
  `ParentNotFound` (`Code::CMD_TARGET_NOT_FOUND`, SACM-CMD-001), `InvalidParent`
  (`Code::CMD_INVALID_PARENT`, SACM-CMD-005), `InvalidId` (`Code::ID_INVALID`,
  SACM-ID-002) and `DuplicateId` (`Code::CMD_DUPLICATE_ID`, SACM-CMD-004).
  `impl From<&ElementId> for ElementId` [LIB-EDIT-001].
- `xmi::read` reads package content: an AssuranceCasePackage in `assuranceCasePackage`, an
  ArgumentPackage in `argumentPackage` or `argumentElement`, and a Claim in
  `argumentElement`, each with its `xmi:id` and name, in document order. A child's class
  is the one its `xmi:type` or `xsi:type` names, or its role's concrete declared type. So
  what `xmi::write` writes for a document built with `edit::apply` reads back. Every other
  child, attribute or text in a package stays `Unsupported`, at its location
  [SACM23-XMI-016]. Every read refusal now carries the line and column of what it refuses
  (an element's `<`, an attribute's name, text's first character that is not literal
  white space); only the parser's capacity limits have none [SACM23-XMI-010].
- `xmi::read` and `xmi::write` read and write a Claim's `assertionDeclaration`. `read`
  takes the six literals exactly and refuses any other value as
  `ReadError::InvalidLiteral` (SACM-ENUM-001), never as `asserted`. `write` writes every non-default
  declaration after `xmi:id` and omits `asserted`, so `WriteError::Unsupported` is
  removed [SACM23-XMI-017, LIB-API-010].
- `compare::semantic_compare` lists every `Difference` in SACM content between two
  documents: presence, kind, parent, name, `assertionDeclaration`, and the order of roots
  and children; generated-id counters are not compared. `Document: PartialEq + Eq` is true
  when there is none [LIB-RT-001]. Load, save and load gives an equal document and the same
  bytes for every valid fixture, and documents built with `edit::apply` round-trip
  [LIB-RT-002, LIB-RT-003].
- `edit::Mutation` reports the operation and the ids created, changed and deleted, and
  `Operation::operation_name()` gives a stable name for logs [LIB-EDIT-003];
  `edit::apply` is all or nothing: on success the document is structurally valid; on a
  refusal it is unchanged (`==` to before, and the same ids are generated next)
  [LIB-EDIT-002].
- `xmi::Mode`; `ReadOptions`/`WriteOptions::with_mode`; `ReadOutput::mode`; read
  diagnostics SACM-XMI-010 (`xsi:type`) and SACM-XMI-011 (`xmi:version`, Info). A
  tolerant read, the default, opens every well-formed file with SACM 2.3 package roots:
  it refuses only a DOCTYPE, malformed XML, `InvalidRoot`, `NoPackage`, `DuplicateId`
  and `Unsupported` for an encoding, a limit, a root outside SACM 2.3 or XMI 2.5.1, or
  an XMI-namespace document element, and keeps every other construct a strict read refuses as preserved content, with one
  Warning at the strict refusal's location (SACM-ENUM-001, SACM-MULT-001, SACM-XMI-005,
  SACM-ID-002 with a generated id), and never reports an Error [LIB-COMPAT-001].
- `xmi::ReadError::DuplicateId` (SACM-ID-001, new `Code::ID_DUPLICATE`), giving the
  location of every element that carries the repeated `xmi:id`, and `xmi::ReadError::InvalidId`
  (SACM-ID-002) for an empty `xmi:id`; `ReadError::locations()` gives every location a
  refusal names [SACM23-VAL-001, SACM23-VAL-020].
- `validation::validate` reports each element whose id is not an XML NCName as a
  SACM-ID-002 Error [SACM23-VAL-020].
- `sacm validate <file> [--strict | --tolerant] [--format human|json]`, with exit codes 0, 1 and 2
  and a versioned JSON report (`"format":1`) [LIB-CLI-002].
- `sacm roundtrip <file> [--strict] [-o <out>]`: read, save strictly, reload and compare;
  `-o` writes the strict output only when the round trip is faithful [LIB-CLI-003].
- `xmi::WriteError::Invalid`, a strict write's refusal of output that would not be
  schema-valid: one SACM-ID-002 per `xmi:id` that is no `NCName` and one SACM-REF-001 per
  reference to no `xmi:id` the output writes, in `Diagnostic` order; and
  `WriteError::diagnostics()`, which gives every such Error. A strict write never runs
  `validate` [LIB-COMPAT-007].
- `xmi::read` reads the element form of a name's `lang` and `content` and a Claim's
  `assertionDeclaration`; a value given more than once, even with equal values, is
  `ReadError::RepeatedValue` (SACM-MULT-001) [SACM23-XMI-014].
- `xmi::read` reads an element without an `xmi:id`, a root or a child, with a generated
  id in the edit scheme (`claim_1`) that no `xmi:id` in the file holds, reported as
  SACM-XMI-004 Info (`Code::XMI_MISSING_ID`); `xmi::write` writes it, and later edits
  generate above it [SACM23-XMI-020].
- `xmi::read` reads an `xmi:id` with XML white space around an NCName trimmed, reported as
  SACM-ID-004 Info (`Code::ID_TRIMMED`); `xmi::write` writes it trimmed [SACM23-XMI-025].
- `xmi::read` keeps the `xmi:id` of a `name` as `SacmElement::name_id()`, in the
  document's id space; `xmi::write` writes it back first on `name`, and
  `semantic_compare` reports `Difference::NameId`; `validate` reports one that is not an
  NCName as SACM-ID-002 on the owner [SACM23-XMI-022].
- `ReadError::InvalidLiteral` (SACM-ENUM-001, `Code::ENUM_INVALID_LITERAL`): a strict
  read's refusal of an `assertionDeclaration` that is not one of the six literals. A
  tolerant read keeps the value as preserved content, with SACM-ENUM-001 as a Warning,
  and holds `asserted`; the value counts as the property's first occurrence, and
  `SetAssertionDeclaration` and `SetFlag` replace a preserved attribute of their
  property's name [SACM23-VAL-006].
- Descriptions, implementation constraints, notes and tagged values on every
  ModelElement (ADR 0025): `SacmElement::descriptions()`,
  `implementation_constraints()`, `notes()` and `tagged_values()` filter the one ordered
  child list; `content()` and, on a TaggedValue, `key()` are `None` or a
  `MultiLangString` (absent and empty kept apart), with `content_id()`,
  `content_value_ids()`, `key_id()` and `key_value_ids()` for the `xmi:id`s of the role
  and value elements. `xmi::read` reads them (each `value` like a `name`; a second
  `content` or `key` is `RepeatedValue`; the single-language shorthand stays
  `Unsupported` until LIB-COMPAT-008); `xmi::write` writes `content` before `key`;
  `semantic_compare` reports `Difference::Content`, `ContentId`, `ContentValueIds`,
  `Key`, `KeyId` and `KeyValueIds`; `WriteAttribute` gains `ContentValueLang`,
  `ContentValueContent`, `KeyValueLang` and `KeyValueContent`. `edit::CreateDescription`,
  `CreateImplementationConstraint`, `CreateNote` and `CreateTaggedValue` create them
  under any ModelElement. `impl From<LangString> for MultiLangString`
  [SACM23-BASE-004, SACM23-BASE-005].
- `SacmElement::gid()`: every element's `gid`, held exactly (absent, `""` and others
  differ). `xmi::read` reads it as an attribute or a value element (given twice:
  `RepeatedValue`); `xmi::write` writes it after `xmi:id`; `semantic_compare` reports
  `Difference::Gid`; `WriteAttribute::Gid` [SACM23-BASE-006].
- `validation::validate` reports an empty `content` or `key`, each language that two or
  more values of one share, and a TaggedValue with no `key`, as SACM-MULT-001 Errors on the
  UtilityElement; and a `content`, `key` or `value` `xmi:id` that is not an NCName as
  SACM-ID-002 [SACM23-VAL-010, SACM23-VAL-020].
- `validation::validate` reports each non-empty `gid` that two or more elements hold as a
  SACM-ID-003 Error (`Code::GID_DUPLICATE`) on every holder; a ModelElement with two or
  more descriptions as a SACM-MULT-001 Warning (clause 8.6
  `description:Description[0..1]`); and an ArgumentPackage that nests a plain
  ArgumentPackage and also holds another argumentation element as one SACM-PKG-001 Error
  (`Code::PACKAGE_CONTENT_INVALID`) per such element [SACM23-VAL-002, SACM23-VAL-011,
  SACM23-VAL-012].
- A test checks that every code the crate can emit has its catalog row and every live
  row names its constant; the catalog's status note states that rule [LIB-DIAG-002].
- `SacmElement::is_citation()`, `cited_element()`, `is_abstract()` and `abstract_form()`:
  every element's clause 8.2 flags (false unless the file sets them `true`) and references, held as the
  ids they name (ADR 0026). `xmi::read` reads a flag as an `xsd:boolean` (`true`, `1`,
  `false`, `0`), as an attribute or a value element, and refuses any other value as
  `ReadError::InvalidLiteral`; it reads a reference as an attribute of white-space
  separated ids or as an element with `xmi:idref` or `href="#id"`, and keeps one that
  names no element. Generated ids, in the reader and in edits, skip every referenced id.
  `xmi::write` writes the flags when true and the references as attributes, after
  `gid`; `semantic_compare` reports `Difference::IsCitation`, `IsAbstract`,
  `CitedElement` and `AbstractForm` [SACM23-XMI-005, SACM23-XMI-015, SACM23-BASE-007,
  SACM23-BASE-008].
- `validation::validate` reports each reference that names no element as a SACM-REF-001
  Error (`Code::REF_DANGLING`); an element with a `citedElement` but no `isCitation` true
  as a SACM-CITE-001 Error (`Code::CITATION_INVALID`); and, with
  `Code::ABSTRACT_FORM_INVALID` (SACM-ABS-001), an abstract element with an
  `abstractForm` as an Error, and an `abstractForm` target that is not abstract or of
  another kind as a Warning [SACM23-VAL-003, SACM23-VAL-007, SACM23-VAL-008].
- Asserted relationships (ADR 0027). `xmi::read` and `xmi::write` handle
  `ArgumentReasoning`, `AssertedInference` and `AssertedContext` in an `ArgumentPackage`.
  `SacmElement::meta_claims` gives an Assertion's `metaClaim` ids, the first multi-valued
  reference end, in order with repeats. `SacmElement::sources`, `target`, `reasoning` and
  `is_counter` give an asserted relationship's ends and flag, and `structure` gives an
  `ArgumentReasoning`'s package. They are written class by class, superclass first: an
  Assertion's after `assertionDeclaration`. `edit::CreateArgumentReasoning`, `edit::CreateAssertedInference` and
  `edit::CreateAssertedContext` create them, refusing no source as
  `EditError::TooFewReferences` (SACM-MULT-001), an id that holds white space as
  `EditError::InvalidReference` (SACM-ID-002), and an id that names no element as
  `EditError::ReferenceNotFound` (SACM-CMD-001). `semantic_compare` reports
  `Difference::IsCounter`, `MetaClaims`, `Reasoning`, `Sources`, `Target` and `Structure`.
  `validation::validate` reports a relationship with no source, and one with no target,
  as SACM-MULT-001 Errors [SACM23-ARG-004, SACM23-ARG-005, SACM23-ARG-007,
  SACM23-ARG-008, SACM23-ARG-010, SACM23-VAL-005].
- Artifact references, argument groups and the artifact relationships (ADR 0028):
  `SacmElement::referenced_artifact_elements` and `group_members` give an
  `ArtifactReference`'s and an `ArgumentGroup`'s ends. `edit::CreateArtifactReference`,
  `CreateArgumentGroup`, `CreateAssertedEvidence`, `CreateAssertedArtifactSupport` and
  `CreateAssertedArtifactContext` create them. `semantic_compare` reports
  `Difference::ReferencedArtifactElements` and `GroupMembers`. `validation::validate`
  reports a reference whose target has the wrong kind as SACM-REF-002
  (`Code::REF_WRONG_TYPE`), and an id repeated in one multi-valued end as a SACM-MULT-001
  Warning [SACM23-ARG-006, SACM23-ARG-009, SACM23-ARG-011, SACM23-ARG-012,
  SACM23-ARG-013, SACM23-VAL-004, SACM23-VAL-016, SACM23-VAL-022].
- Edits that change an existing element (ADR 0029):
  - `edit::SetName`, `SetGid`, `SetFlag`, `SetAssertionDeclaration`, `SetContent` and
    `SetKey` set a feature;
  - `SetReferences`, `AddReference` and `RemoveReference` edit any reference end;
  - `ReorderElements` reorders an element's children or the document's roots.

  `model::Flag` and `model::ReferenceEnd` (both `#[non_exhaustive]`, with a public `name`)
  name what is edited. A refusal leaves the document
  unchanged: the new `EditError` variants are `ElementNotFound`, `FeatureNotApplicable`
  (SACM-CMD-008, `Code::CMD_FEATURE_NOT_APPLICABLE`), `TooManyReferences`,
  `ReferenceNotHeld`, `ChildNotFound` and `InvalidOrder` (SACM-CMD-009,
  `Code::CMD_INVALID_ORDER`). `Mutation::changed_ids` names the edited element, and is empty
  when nothing changed [LIB-EDIT-008, LIB-EDIT-009, LIB-EDIT-010, LIB-EDIT-012].
- `ElementKind::may_contain` (a `const fn`) and `ElementKind::child_kinds` answer which
  kinds SACM 2.3 lets an element contain, from the containment table the edits use
  [LIB-API-012].
- `sacm` builds for `wasm32-unknown-unknown`, checked in CI, and a lint keeps file-system,
  thread, process, environment, network and clock APIs out of the library [LIB-QA-017].
- `edit::check(&Document, &Operation)` answers whether `edit::apply` would accept an
  operation, with the same `EditError` if not, and never changes the document. `check`
  and `apply` share one code path [LIB-EDIT-013].
- The optional `serde` feature: `Serialize` and `Deserialize` for `Operation` and every
  operation struct, `Mutation`, `EditError`, `Diagnostic`, and the values they carry
  (`ElementId`, `ElementKind`, `LangString`, `MultiLangString`, `AssertionDeclaration`,
  `Flag`, `ReferenceEnd`, `Code`, `Severity`, `SourceLocation`). The JSON shape is
  documented in ADR 0030; it follows semver, not a frozen format. Unknown keys, names and
  codes are refused. `Document` is not serialized [LIB-API-006].
- `edit::preview(&Document, &Operation)` returns the `Preview` of any operation: the
  `Mutation` `apply` would return and, for a delete, each `DanglingReference` (holder, end,
  id, whether it crosses a package boundary), without changing the document. A `Preview`
  serializes but does not deserialize [LIB-EDIT-004].
- `edit::DeleteElement` deletes an element of any kind and what it contains, refusing a
  referenced element unless its `ReferenceDeletePolicy` is `DeleteReferencingRelationships`
  or `ScrubReferences` (`EditError::Referenced`, `Code::CMD_DELETE_REFERENCED`); a
  successful delete never leaves a dangling reference. A delete does not rewind the id
  counters, so generation stays deterministic; a freed id may be used again
  [LIB-EDIT-005].
- `DeleteElement` refuses a non-empty element unless its `PackageDeletePolicy` is
  `DeleteRecursively` (`EditError::NotEmpty`, `Code::CMD_PACKAGE_NOT_EMPTY`), and a
  reference across a package boundary unless its `CrossPackageReferencePolicy` is
  `ApplyReferencePolicy` (`EditError::CrossPackageReference`,
  `Code::CMD_EXTERNAL_REFERENCES`) [LIB-EDIT-006].
- `Document::revision()` returns an opaque `model::Revision`, new for every document and
  every successful edit, and `edit::apply_preview` refuses a preview taken at another
  revision (`EditError::PreviewExpired`, `Code::CMD_PREVIEW_EXPIRED`) [LIB-EDIT-007].
- `edit::batch(&mut Document, |tx| ...)` and `edit::apply_all` apply several operations as
  one atomic change; a later operation can use an id an earlier one generated. The
  `BatchMutation` holds each `Mutation` and the net created, changed and deleted ids, and
  a `BatchError` names the refused operation by index [LIB-EDIT-014].
- A tolerant `xmi::read` reads the assurance-forge C++ library's one-to-one name and text
  shapes (`name="…"`, text in a `name` or `value`, a `name` holding one `value` or one
  `content lang`, the single-language shorthand of a `content` or `key`, `content=` or
  text on a UtilityElement, `key=`/`value=` on a TaggedValue) as their standard form, with
  one SACM-XMI-012 Warning each (`Code::XMI_LEGACY_SHAPE`); a strict read refuses them as
  `ReadError::LegacyShape` [LIB-COMPAT-008].
- `xmi::read` reads `xsi:nil` on a value element as an `xsd:boolean`: true reads as the
  property not written, false reads the content, and another value is
  `ReadError::InvalidLiteral` [SACM23-XMI-026].
- An untyped child under an abstract role, such as an `argumentElement` with no
  `xmi:type`, is refused by a strict read as `ReadError::MissingType`
  (`Code::XMI_MISSING_TYPE`, SACM-XMI-005); a tolerant read keeps it (LIB-COMPAT-001)
  [LIB-COMPAT-010].
- `validation::validate` warns, as SACM-CITE-002 (`Code::AS_CITED_INVALID`), about an
  Assertion declared `asCited` that is not a citation of an Assertion [SACM23-VAL-019].
- `validation::validate` reports an `implementationConstraint` on an element whose
  `isAbstract` is not true as a SACM-ABS-001 Error [SACM23-VAL-021].
- Preserved content: `xmi::read` keeps, in both modes, content that is not SACM 2.3 where
  it stands (an attribute the class lacks, a foreign or unknown child, a child whose type
  the role does not admit, a defect inside a `name`, `content`, `key`, value or reference
  element, text in element-only content, a root's type naming another class) exactly as
  written, with one SACM-XMI-003 per construct (`Code::XMI_UNKNOWN_CONTENT`; Warning
  tolerant, Error strict). `SacmElement::has_preserved_content` tells a holder; a
  tolerant `xmi::write` writes it back where it stood; `==` and `semantic_compare`
  compare it exactly (`Difference::PreservedContent`); its ids stay in the id space, and
  `validation::validate` reports a reference to one as SACM-REF-003
  (`Code::REF_PRESERVED_TARGET`, a Warning). A document holds preserved content outside
  its elements too: in tolerant mode, the attributes, text and XMI-namespace children of
  `xmi:XMI` and a root of a kind not read yet, placed among the roots, which root
  create, delete and reorder keep in place; `Document::has_preserved_content` tells it,
  `Difference::DocumentPreservedContent` compares it, and a tolerant `xmi::write`
  writes it back on and in `xmi:XMI`, which it then always writes. A preserved attribute
  whose prefix `sacm` or `xmi` is bound elsewhere is kept under the first unbound `nsN`
  [LIB-COMPAT-003].
- A strict `xmi::write` refuses a document that holds preserved content as
  `WriteError::PreservedContent { elements }` (SACM-XMI-006,
  `Code::XMI_STRICT_SAVE_REFUSED`), before the schema check; `elements` is empty when
  only the document holds it [SACM23-XMI-008].
- Every `ReadError::Unsupported` reason ends with one label that says why it is refused:
  `(a limit of this version; ROW reads it)`, `(a limit of this version)`, `(strict mode
  never reads it; ROW is to read it in tolerant mode)` or `(a defect in the input)`, from one
  list that an xtask test gates. The same list says what a tolerant read does with each
  entry: it keeps conforming content not read yet as preserved content with the new
  SACM-XMI-014 (`Code::XMI_UNREAD_CONTENT`), whose message ends `; ROW reads it`, a
  cross-document `href` with SACM-XMI-007 (`Code::XMI_EXTERNAL_HREF`), now live, and a
  defect with SACM-XMI-003, and refuses only the limits, a root outside SACM 2.3 or
  XMI 2.5.1, and an XMI-namespace document element [LIB-API-011].
- `cargo xtask fetch-spec` also fetches the normative XMI.xsd of XMI 2.5.1, hash-pinned
  [LIB-TOOL-021].
- `xmi::read` and `xmi::write` handle `ArtifactPackage` (as a root, in an
  AssuranceCasePackage's `artifactPackage` role and nested in `artifactElement`),
  `Technique`, `Participant`, `ArtifactGroup` and `Property` (in `artifactElement`, and in
  an asset's `property` role) [SACM23-ART-001, SACM23-ART-003, SACM23-ART-008,
  SACM23-ART-009, SACM23-ART-011].
- `edit::CreateArtifactPackage` (a root or a child), `CreateProperty`, `CreateTechnique`,
  `CreateParticipant` and `CreateArtifactGroup` (with its members) [SACM23-ART-001,
  SACM23-ART-003, SACM23-ART-008, SACM23-ART-009, SACM23-ART-011].
- `ReferenceEnd::ArtifactGroupMember` (`artifactElement`), an ArtifactGroup's member end,
  and `Difference::ArtifactGroupMembers` [SACM23-ART-011].
- `SacmElement::properties()`: an artifact asset's `property` children [SACM23-ART-003].
- `ReadOutput::has_errors()`: whether a read reported any Error, so an application can
  tell a strict read's Errors from a clean or warnings-only read without scanning the
  diagnostics [LIB-API-016].
- `cargo xtask fetch-corpus`: downloads 15 SACM files written by other tools (Assurance
  Forge, ACEditor, Epsilon, the EMF SACM implementation, ATL, DEIS ODE), pinned by SHA-256,
  into the git-ignored `third_party/corpus/`. A test reads each in both modes, validates it
  and writes it back tolerantly, and `docs/compat/corpus.md` records the results and the
  row each gap belongs to [LIB-COMPAT-015].
- `xmi::read` and `xmi::write` handle `Artifact` (with `version` and `date`), `Event`
  (`date`), `Resource` (`location`) and `Activity` (`startTime`, `endTime`) in an
  ArtifactPackage's `artifactElement`, with their `property` children [SACM23-ART-002,
  SACM23-ART-004, SACM23-ART-005, SACM23-ART-006, SACM23-ART-007].
- `SacmElement::version()`, `date()`, `start_time()`, `end_time()`, `location()`,
  `location_id()` and `location_value_ids()`. A date-typed value is held exactly as
  written: SACM 2.3 gives `date` no lexical form, and its XML Schema types it `xsd:string`
  [SACM23-ART-002, SACM23-ART-005].
- `edit::CreateArtifact`, `CreateEvent`, `CreateResource` and `CreateActivity`, and
  `SetVersion`, `SetDate`, `SetLocation`, `SetStartTime` and `SetEndTime`
  [SACM23-ART-002, SACM23-ART-004, SACM23-ART-005, SACM23-ART-006, SACM23-ART-007].
- `Difference::Version`, `Date`, `Location`, `LocationId`, `LocationValueIds`,
  `StartTime` and `EndTime`; `WriteAttribute::Version`, `Date`, `StartTime`, `EndTime`,
  `LocationValueLang` and `LocationValueContent` [SACM23-ART-002, SACM23-ART-004,
  SACM23-ART-005].
- `edit::MoveElement` moves an element, with everything it contains, under another
  parent or to the roots, after the last child or at a position (`with_position`). Ids,
  references and preserved content are unchanged. New refusals: `EditError::InvalidRoot`
  and `EditError::ParentInSubtree` (SACM-CMD-005), and `EditError::PositionOutOfRange`
  (SACM-CMD-009) [LIB-EDIT-017].
- `edit::CopyElements` copies elements of another `Document`, with everything they
  contain, under a parent or as roots. A clashing id gets a generated one, and the
  copies' references follow the new ids. `source_ids()` pairs with
  `Mutation::created_ids()`. It works with `check`, `preview` and batches, and it is the
  one operation serde does not serialize [LIB-EDIT-018].
- `Document::referrers` and `model::Referrer` list every typed reference that names an
  id: its holder, end and position, in document order [LIB-API-014].
- `SacmElement::flag` and `SacmElement::references` are public. Also new: `Flag::ALL`,
  `Flag::applies_to`, `ReferenceEnd::ALL`, `ReferenceEnd::applies_to` and
  `ReferenceEnd::bounds` [LIB-API-015].
- `xmi::read` and `xmi::write` handle `ArtifactAssetRelationship` in an
  ArtifactPackage's `artifactElement`, with its `source` and `target` ends and
  `property` children; `validate` reports one with no `source` or no `target`
  (SACM-MULT-001, clause 12.14) [SACM23-ART-010].
- `ReferenceEnd::ArtifactSource` and `ArtifactTarget` (XMI `source` and `target`; serde
  `artifactSource` and `artifactTarget`), `SacmElement::targets()`,
  `edit::CreateArtifactAssetRelationship`, and `Difference::ArtifactSources` and
  `ArtifactTargets` [SACM23-ART-010].
- `Code::XMI_SPELLING_VARIANT` (SACM-XMI-013): a tolerant `xmi::read` reads
  `ArtifactAssertedRelationship`, an Event's `occurece` and `argumentationElement` as
  `ArtifactAssetRelationship`, `date` and `argumentElement`, with one Warning each;
  `xmi::write` writes the resolved names [SACM23-XMI-009].
- The `edit` docs show undo and redo by whole-document copies, with a tested recipe; a
  restored copy is equal and keeps its revision and id counters (ADR 0037) [LIB-EDIT-011].

### Changed

- `xmi::read` reads `ArtifactAssetRelationship`, which a strict read refused as
  `Unsupported` and a tolerant read kept with SACM-XMI-014 [SACM23-ART-010].
- `SacmElement::sources()` also answers for an ArtifactAssetRelationship, and
  `DeleteReferencingRelationships` and `ScrubReferences` delete ArtifactAssetRelationships
  as they delete asserted relationships [SACM23-ART-010].
- `ReadError::RepeatedValue` gains `kind`, and its message calls a property single-valued
  only when the end that applies to the element's kind is [SACM23-ART-010].
- `EditError::FeatureNotApplicable` names the other class when the element has an end of
  the same name, such as `the AssertedInference "i1" has no source of an
  ArtifactAssetRelationship` [SACM23-ART-010].
- A strict `xmi::read` keeps `ArtifactAssertedRelationship`, `occurece` and
  `argumentationElement` as preserved content with a SACM-XMI-013 Error, where it refused
  them as `Unsupported`; a tolerant read reads them, where it kept them with SACM-XMI-014
  [SACM23-XMI-009].
- `xmi::read` reads `Artifact`, `Event`, `Resource` and `Activity`, which a strict read
  refused as `Unsupported` and a tolerant read kept with SACM-XMI-014. An Event's
  `occurece` (the ptc/22-03-13 spelling of `date`) is refused by a strict read, and kept
  by a tolerant read with SACM-XMI-014, until SACM23-XMI-009 [SACM23-ART-007].
- `validation::validate` checks a Resource's `location` as it checks every
  MultiLangString (SACM-MULT-001, SACM-ID-002) [SACM23-ART-005, SACM23-VAL-010,
  SACM23-VAL-020].

- `sacm validate` reads strictly by default: a file is `valid` only when a strict read
  accepts it and no diagnostic is an Error, and a file that a strict read refuses but a
  tolerant read opens is `invalid` (exit 1, with the refusal on stdout), no longer
  unreadable (exit 2). `--tolerant` reads in tolerant mode and reports `readable`, never
  `valid`, in its summary and in the JSON `result`. `--strict` names the default, and
  giving both is a usage error (ADR 0023 amendment; the maintainer, 2026-11-16)
  [LIB-CLI-002].
- A tolerant `xmi::read` returns a document where it refused one before: for a value
  that is no literal, a value given twice, an untyped child under an abstract role, an
  empty `xmi:id`, content on `xmi:XMI`, a rebound `sacm` or `xmi` prefix, and conforming
  content not read yet, each kept as preserved content with a Warning. A strict read is
  unchanged, and each entry below that says `xmi::read` refuses something now describes a
  strict read (ADR 0035; the maintainer, 2026-11-16) [LIB-COMPAT-001, LIB-COMPAT-003,
  LIB-API-011, SACM23-VAL-006].
- A strict `xmi::write` checks schema validity only and never runs `validate`: it writes
  a document with model Errors, such as an element with no name or a relationship with no
  target, that it refused as `WriteError::Invalid` before; that variant now holds the
  schema errors, SACM-ID-002 and SACM-REF-001, and its `Display` counts "schema errors".
  No validation rule refuses a strict write: SACM-ABS-001, SACM-CITE-001, SACM-ID-003,
  SACM-MULT-001, SACM-PKG-001 and SACM-REF-002 are reported by `validate` only (ADR 0035
  D4; the maintainer, 2026-11-16) [LIB-COMPAT-007, SACM23-XMI-008].
- `WriteError::PreservedContent`'s `Display` no longer says "that is not SACM 2.3", and
  says when the document alone holds the content [SACM23-XMI-008].
- The `serde` JSON shape follows the crate's semver until 1.0, rather than being frozen: a
  change to a key, tag or meaning ships only in a release that bumps the minor version
  (ADR 0030 amendment 1; the maintainer, 2026-11-16) [LIB-API-006].
- `ReadError::RepeatedValue` locates each id of a repeated reference attribute at its own
  character, and its `Display` names at most the first two locations and the count;
  `locations()` still lists every occurrence [SACM23-XMI-005].
- A containment role element in reference form (a proxy) is refused as not read in this
  release: with `xmi:idref` under SACM23-XMI-019, with an unqualified `href` under
  SACM23-XMI-027, typed or untyped, under an abstract or a concrete role. An untyped one
  under an abstract role was `MissingType`, and a typed one, or one under a concrete role,
  with `href` named SACM23-XMI-019 [LIB-COMPAT-010, LIB-API-011].
- An empty CDATA section under `xsi:nil="true"` counts as no content [SACM23-XMI-026].
- A root in another SACM revision's namespace is refused as a tolerance not built yet
  (LIB-COMPAT-004), not as conforming input not read yet [LIB-API-011].
- Replacing a large `content` or `key` is no longer quadratic in the number of its ids: it
  keeps them in n log n time [LIB-EDIT-012].
- The label of a tolerance refusal, input that only tolerant mode is to read, is now
  `(strict mode never reads it; ROW is to read it in tolerant mode)`, not `(a defect in the
  input; ROW is to read it in tolerant mode)`: such input need not be defective, as a
  document in another SACM namespace is not (D-NS-PIN) [LIB-API-011].
- `xmi::read` keeps content that is not SACM 2.3 where it stands as preserved content,
  with SACM-XMI-003, which is an Error in strict mode, instead of refusing it as
  `Unsupported`: a strict read may report an Error where it refused before, and a strict
  `xmi::write` refuses such a document as `WriteError::PreservedContent`
  [LIB-COMPAT-003, SACM23-XMI-008].
- `WriteError::code()` is `Some(Code::XMI_STRICT_SAVE_REFUSED)` for the new
  `WriteError::PreservedContent`; it stays `None` for the other variants [SACM23-XMI-008].

- `xmi::read` refuses an untyped child under an abstract role as the coded
  `ReadError::MissingType` instead of `Unsupported`, and a strict read refuses the C++
  name and text shapes as `ReadError::LegacyShape` instead of `Unsupported`
  [LIB-COMPAT-010, LIB-COMPAT-008].
- Every create with reference ends, including `CreateArgumentReasoning`,
  `CreateAssertedInference` and `CreateAssertedContext`, refuses an end whose target does
  not conform to the end's declared type as `EditError::WrongReferenceType`
  (SACM-REF-002), after `ReferenceNotFound` [SACM23-VAL-004].
- `SacmElement::group_members()` also answers for an ArtifactGroup, with its
  `artifactElement` ids [SACM23-ART-011].
- `xmi::read` reads an `ArtifactPackage` and its Techniques, Participants, Properties and
  ArtifactGroups, which it refused as `Unsupported` before. The other kinds it can hold
  are refused naming their own rows (SACM23-ART-004 to -007, -010, SACM23-TERM-002 and
  -004 to -006), the type `ArtifactAssertedRelationship` names SACM23-XMI-009, and an
  ArtifactPackage's `interface` names SACM23-ART-012 [SACM23-ART-001].
- `xmi::read` reads `ArtifactReference`, `ArgumentGroup`, `AssertedEvidence`,
  `AssertedArtifactSupport` and `AssertedArtifactContext` in an `ArgumentPackage`, which
  it refused as `Unsupported` before [SACM23-ARG-006, SACM23-ARG-009, SACM23-ARG-011,
  SACM23-ARG-012, SACM23-ARG-013].
- `xmi::write` no longer writes `xmi:version`: the XMI namespace identifies the version
  (XMI 2.5.1 §7.5.3) [SACM23-XMI-024].
- `xmi::write` is strict by default: it refuses output that would not be schema-valid as
  `WriteError::Invalid`, and preserved content; write it as held with
  `WriteOptions::default().with_mode(Mode::Tolerant)` [LIB-COMPAT-007].
- `xmi::read` refuses a duplicate `xmi:id` as `ReadError::DuplicateId` and an empty one
  as `ReadError::InvalidId`, coded refusals instead of `Unsupported`
  [SACM23-VAL-001, SACM23-VAL-020].
- `xmi::read` refuses an `assertionDeclaration` that is not a literal as
  `ReadError::InvalidLiteral` (SACM-ENUM-001) instead of `Unsupported` [SACM23-VAL-006].
- `xmi::read` refuses a second `name` as `ReadError::RepeatedValue` (SACM-MULT-001) with
  property `name`, instead of `Unsupported` [SACM23-XMI-021].
- An `id` attribute that is not `xmi:id` (`id`, `o:id`) is refused as `Unsupported` at
  the attribute's name, now that an element without an `xmi:id` is read
  [SACM23-XMI-020].
- `xmi::read` refuses a name's `lang` or `content`, or a Claim's `assertionDeclaration`,
  given both as an attribute and as an element as `ReadError::RepeatedValue`
  (SACM-MULT-001) instead of `Unsupported` [SACM23-XMI-014].
- A strict `xmi::write` refuses a document holding an id that is not an NCName, such as
  `1c` or `a b`, as `WriteError::Invalid` with SACM-ID-002, a schema error. Write it as
  held in tolerant mode [SACM23-VAL-020].
- A strict `xmi::write` refuses a document with a reference that names no `xmi:id` the
  output writes, as `WriteError::Invalid` with SACM-REF-001, a schema error. Write it as
  held in tolerant mode [SACM23-VAL-003].
- `xmi::read` reads `ArgumentReasoning`, `AssertedInference` and `AssertedContext` in an
  `ArgumentPackage`, which it refused as `Unsupported` before. `ReadError::RepeatedValue`
  for `metaClaim` or `source` given both as an attribute and as elements says so
  [SACM23-ARG-004, SACM23-ARG-005, SACM23-ARG-008, SACM23-ARG-010].
- `xmi::read` reads `isCitation`, `isAbstract`, `citedElement` and `abstractForm`, which
  it refused as `Unsupported` before; a flag that is not a Boolean literal is now the
  coded refusal `ReadError::InvalidLiteral` (SACM-ENUM-001), and two ids on one reference
  end `ReadError::RepeatedValue` (SACM-MULT-001). A cross-document `href` is
  `Unsupported` until SACM23-XMI-027 [SACM23-XMI-005, SACM23-XMI-015, SACM23-BASE-007,
  SACM23-BASE-008].
- `xmi::read` reads descriptions, notes, implementation constraints, tagged values and
  `gid`, which it refused as `Unsupported` before [SACM23-BASE-004, SACM23-BASE-005,
  SACM23-BASE-006].
- Reading a file with many diagnostics locates them in one pass over the text.
- The crate root and README show a load, inspect, edit, validate and save example
  [LIB-DOC-001].
- Simplified the harness: removed CI YAML pin tests and the mutation-exclusions command.
