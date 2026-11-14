# Changelog

All notable changes to this project are documented here. The format follows
[Keep a Changelog](https://keepachangelog.com/en/1.1.0/), and the project adheres to
[Semantic Versioning](https://semver.org/spec/v2.0.0.html). Before 1.0, a breaking
change bumps the minor version and is listed under **Breaking**. A new validation Error
rule is not breaking, but it can make a strict `xmi::write` refuse a document it wrote
before, so each one is listed under **Changed** with its code. A new read diagnostic that
is an Error in strict mode, or a new coded read refusal, is listed the same way.

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
- Error convention (ADR 0011): one `#[non_exhaustive]` error enum per operation.
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
  diagnostics SACM-XMI-010 (`xsi:type`) and SACM-XMI-011 (`xmi:version`, Info)
  [LIB-COMPAT-001].
- `xmi::ReadError::DuplicateId` (SACM-ID-001, new `Code::ID_DUPLICATE`), giving the
  location of every element that carries the repeated `xmi:id`, and `xmi::ReadError::InvalidId`
  (SACM-ID-002) for an empty `xmi:id`; `ReadError::locations()` gives every location a
  refusal names [SACM23-VAL-001, SACM23-VAL-020].
- `validation::validate` reports each element whose id is not an XML NCName as a
  SACM-ID-002 Error [SACM23-VAL-020].
- `sacm validate <file> [--strict] [--format human|json]`, with exit codes 0, 1 and 2
  and a versioned JSON report (`"format":1`) [LIB-CLI-002].
- `sacm roundtrip <file> [--strict] [-o <out>]`: read, save strictly, reload and compare;
  `-o` writes the strict output only when the round trip is faithful [LIB-CLI-003].
- `xmi::WriteError::Invalid`, a strict write's refusal of a document that validation
  reports an Error for, and `WriteError::diagnostics()`, which gives every such Error
  [LIB-COMPAT-007].
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
- `ReadError::InvalidLiteral` (SACM-ENUM-001, `Code::ENUM_INVALID_LITERAL`): the refusal,
  in both modes, of an `assertionDeclaration` that is not one of the six literals
  [SACM23-VAL-006].
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

### Changed

- Every create with reference ends, including `CreateArgumentReasoning`,
  `CreateAssertedInference` and `CreateAssertedContext`, refuses an end whose target does
  not conform to the end's declared type as `EditError::WrongReferenceType`
  (SACM-REF-002), after `ReferenceNotFound` [SACM23-VAL-004].
- The new validation Error rule SACM-REF-002 can make a strict `xmi::write` refuse a
  document it wrote before: one with a reference whose target has the wrong kind, such as
  a `metaClaim` naming an ArgumentReasoning, or an AssertedEvidence,
  AssertedArtifactSupport or AssertedArtifactContext whose ends break §11.15, §11.17 or
  §11.18. Write it as held in tolerant mode [SACM23-VAL-004, SACM23-VAL-016].
- `xmi::read` reads `ArtifactReference`, `ArgumentGroup`, `AssertedEvidence`,
  `AssertedArtifactSupport` and `AssertedArtifactContext` in an `ArgumentPackage`, which
  it refused as `Unsupported` before [SACM23-ARG-006, SACM23-ARG-009, SACM23-ARG-011,
  SACM23-ARG-012, SACM23-ARG-013].
- `xmi::write` no longer writes `xmi:version`: the XMI namespace identifies the version
  (XMI 2.5.1 §7.5.3) [SACM23-XMI-024].
- `xmi::write` is strict by default: it runs `validation::validate` and refuses a
  document with any Error as `WriteError::Invalid`; write it as held with
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
- The new validation Error rule SACM-ID-002 can make a strict `xmi::write` refuse a
  document it wrote before: one holding an id that is not an NCName, such as `1c` or
  `a b`. Write it as held in tolerant mode [SACM23-VAL-020].
- The new validation Error rule SACM-MULT-001 for MultiLangStrings can make a strict
  `xmi::write` refuse a document: one with an empty `content` or `key`, two values in
  one language, or a TaggedValue without a `key`. Write it as held in tolerant mode
  [SACM23-VAL-010].
- The new validation Error rule SACM-ID-003 can make a strict `xmi::write` refuse a
  document it wrote before: one where two elements hold the same non-empty `gid`. Write
  it as held in tolerant mode [SACM23-VAL-002].
- The new validation Error rule SACM-PKG-001 can make a strict `xmi::write` refuse a
  document it wrote before: one with an ArgumentPackage that nests a plain
  ArgumentPackage and also holds a Claim or another argumentation element. Write it as
  held in tolerant mode [SACM23-VAL-012].
- The new validation Error rule SACM-REF-001 can make a strict `xmi::write` refuse a
  document: one with a `citedElement` or `abstractForm` that names no element. Write it
  as held in tolerant mode [SACM23-VAL-003].
- The new validation Error rule SACM-CITE-001 can make a strict `xmi::write` refuse a
  document: one with a `citedElement` on an element whose `isCitation` is not true
  [SACM23-VAL-007].
- The new validation Error rule SACM-ABS-001 can make a strict `xmi::write` refuse a
  document: one with an `abstractForm` on an element whose `isAbstract` is true. Its
  Warnings do not refuse [SACM23-VAL-008].
- The new validation Error rule SACM-MULT-001 for asserted relationships can make a
  strict `xmi::write` refuse a document: one with an asserted relationship that has no
  `source` or no `target`. A tolerant `xmi::write` still writes it as held
  [SACM23-VAL-005].
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
