# Changelog

All notable changes to this project are documented here. The format follows
[Keep a Changelog](https://keepachangelog.com/en/1.1.0/), and the project adheres to
[Semantic Versioning](https://semver.org/spec/v2.0.0.html). Before 1.0, a breaking
change bumps the minor version and is listed under **Breaking**. A strict `xmi::write`
checks schema validity only, so a new validation rule never changes what it writes. A new
strict read refusal, or a new strict write refusal, is listed under **Changed**.

## [Unreleased]

### Added

- The Terminology package of SACM 2.3: TerminologyPackage, Category, Term, Expression and
  TerminologyGroup are read, written, created, edited, compared and validated. A
  TerminologyPackage is read as a root (the Terminology unit of interchange, §2.5), in an
  AssuranceCasePackage, nested, and in an ArtifactPackage [SACM23-TERM-001 to
  SACM23-TERM-006].
- `ReferenceEnd::Category`, `Element`, `Origin` and `TerminologyGroupMember`
  (`terminologyElement`); `SacmElement::categories()`, `expression_elements()`, `origin()`,
  `value()` and `external_reference()`; `group_members()` answers for a TerminologyGroup
  [SACM23-TERM-002 to SACM23-TERM-006].
- `edit::CreateTerminologyPackage`, `CreateCategory`, `CreateTerm`, `CreateExpression`,
  `CreateTerminologyGroup`, `SetValue` and `SetExternalReference`; `Difference::Categories`,
  `ExpressionElements`, `Origin`, `TerminologyGroupMembers`, `Value` and
  `ExternalReference`; `WriteAttribute::Value` and `ExternalReference` [SACM23-TERM-001 to
  SACM23-TERM-006].
- `validate` reports a Term or Expression with no `value` as SACM-MULT-001
  [SACM23-TERM-003].
- A tolerant read reads the §10.4 spelling `TerminologyElement` of a TerminologyPackage's
  role, with a SACM-XMI-013 Warning; a strict read keeps it as preserved content with an
  Error [SACM23-XMI-028].

### Changed

- A strict read no longer refuses a TerminologyPackage, Category, Term, Expression or
  TerminologyGroup as `Unsupported`, and a tolerant read no longer keeps them as preserved
  content with SACM-XMI-014; references to them are type-checked (no SACM-REF-003)
  [LIB-API-011, SACM23-TERM-001].

### Known limits

- Not read yet: `ExpressionLangString` (M4); the interface and binding kinds of the four
  packages, and a package's `interface` end (M5); cross-document references, role
  elements in reference form, and XMI metadata (SACM23-XMI-019, SACM23-XMI-027). A strict
  read refuses them as `ReadError::Unsupported` naming the row that reads them; a tolerant
  read keeps them as preserved content.

## [0.1.0] - 2026-11-17

The first release: `sacm` 0.1.0, a library that reads, validates, edits and writes
OMG SACM 2.3 assurance cases as XMI 2.5.1, and `sacm-cli` 0.1.0, the `sacm` command. It
covers the Assurance Case, Argumentation and Artifact packages of SACM 2.3 and the Base
classes they use. The MSRV is 1.85. The library builds for `wasm32-unknown-unknown`, and
does no file, network, thread, process, environment or clock access [LIB-QA-002,
LIB-QA-017]. Both packages are MIT-licensed, and ship the `LICENSE` and no tests or test
data [LIB-QA-005].

### Added

#### Reading and writing XMI

- `xmi::read(&[u8], &ReadOptions)` reads a document into a `ReadOutput` (the `Document`
  and its diagnostics), or refuses it with a `ReadError`; it never returns a partial
  document. Roots are AssuranceCasePackage, ArgumentPackage and ArtifactPackage, bare or
  in `xmi:XMI`; any other root is `ReadError::InvalidRoot`, and an `xmi:XMI` with no
  package `ReadError::NoPackage` (SACM-XMI-001) [LIB-API-009, SACM23-XMI-001].
- Elements are read under their role names, in document order, with the class their
  `xmi:type` (or `xsi:type`) names or their role's concrete type gives. Names are matched
  by namespace URI, never by prefix. An untyped child under an abstract role is refused by
  a strict read as `ReadError::MissingType` (SACM-XMI-005) [SACM23-XMI-002,
  SACM23-XMI-016, LIB-COMPAT-010].
- Values are read exactly as XML gives them, from an attribute or a value element; given
  twice, even equal, a strict read refuses them as `ReadError::RepeatedValue`
  (SACM-MULT-001). `xsi:nil` is read as an `xsd:boolean`. A literal other than the
  standard's, such as an unknown `assertionDeclaration` or a flag that is not an
  `xsd:boolean`, is refused by a strict read as `ReadError::InvalidLiteral`
  (SACM-ENUM-001) [SACM23-XMI-011, SACM23-XMI-012,
  SACM23-XMI-014, SACM23-XMI-017, SACM23-XMI-021, SACM23-XMI-026, SACM23-VAL-006].
- References are read from an attribute of white-space separated ids, or from elements
  with `xmi:idref` or `href="#id"`, and kept when they name no element [SACM23-XMI-005,
  SACM23-XMI-015].
- Ids: an element without an `xmi:id` gets a generated one (SACM-XMI-004, Info); white
  space around an NCName is trimmed (SACM-ID-004, Info); a repeated `xmi:id` is
  `ReadError::DuplicateId` (SACM-ID-001) in both modes, and an empty one is refused by a
  strict read as `ReadError::InvalidId` and given a generated id by a tolerant one
  (SACM-ID-002). The `xmi:id`s of a `name`, `content`, `key`, `location` and `value` are
  kept, in the document's id space [SACM23-XMI-020, SACM23-XMI-022, SACM23-XMI-025,
  SACM23-VAL-001, SACM23-VAL-020].
- XML safety: a DOCTYPE is refused (`ReadError::Doctype`, SACM-SEC-001), and malformed or
  truncated XML and invalid UTF-8 are `ReadError::Malformed` (SACM-XML-001), including
  what the parser lets through; every not-well-formed XML 1.0 (fifth edition) and
  Namespaces 1.0 case of the W3C XML Conformance Test Suite 20130923 is refused. UTF-16 with a byte order mark is read [LIB-SEC-001, LIB-SEC-002,
  LIB-SEC-005, LIB-SEC-006, SACM23-XMI-018].
- Every refusal but one for the parser's capacity limits carries the line and column of
  what it refuses, and `ReadError::locations()` gives them all; every read diagnostic
  carries one too. A `ReadError::Unsupported` reason ends with a label that says why: a
  limit of this version, input for a tolerance, or a defect [SACM23-XMI-010,
  SACM23-XMI-023, LIB-API-011].
- Two modes (`xmi::Mode`, `with_mode`). A strict read takes SACM 2.3 as specified. A
  tolerant read, the default, opens every well-formed file with SACM 2.3 package roots
  except one with a DOCTYPE, a repeated `xmi:id`, an encoding it cannot decode or input
  past the parser's limits: it keeps what else a strict read refuses as preserved content
  with one Warning (an empty `xmi:id` instead gets a generated id, SACM-ID-002), reads
  `xsi:type` (SACM-XMI-010) and the assurance-forge C++ library's one-to-one name and text
  shapes (SACM-XMI-012; strictly `ReadError::LegacyShape`), and never reports an Error.
  `xmi:version` is ignored (SACM-XMI-011, Info). `ReadOutput::has_errors()` tells a read
  with Errors [LIB-COMPAT-001, LIB-COMPAT-008, LIB-API-016].
- Preserved content: content that is not SACM 2.3 is kept exactly as written where it
  stands, with one SACM-XMI-003 per construct (a Warning tolerant, an Error strict);
  conforming content this release does not read yet is kept by a tolerant read with
  SACM-XMI-014, and a cross-document `href` with SACM-XMI-007.
  `SacmElement::has_preserved_content` and `Document::has_preserved_content` tell it, a
  tolerant `xmi::write` writes it back where it stood, and `semantic_compare` compares it
  [LIB-COMPAT-003, LIB-API-011].
- The names formal/23-05-08 and ptc/22-03-13 spell differently: a tolerant read reads the
  type `ArtifactAssertedRelationship`, an Event's `occurece` and `argumentationElement` as
  `ArtifactAssetRelationship`, `date` and `argumentElement`, with a SACM-XMI-013 Warning;
  a strict read keeps them as preserved content with a SACM-XMI-013 Error
  [SACM23-XMI-009].
- `xmi::write(&Document, &WriteOptions)` writes deterministically: UTF-8, LF, two spaces
  per level, the `sacm` and `xmi` namespaces on the document element, one root bare or
  several in `xmi:XMI`, children under their role names in document order, `xmi:type`
  where the role does not give the class, an `xmi:id` on every element, and no
  `xmi:version` [SACM23-XMI-003, SACM23-XMI-004, SACM23-XMI-006, SACM23-XMI-007,
  SACM23-XMI-013, SACM23-XMI-024].
- A strict write, the default, checks only that the output is schema-valid, and never
  runs `validate`: an id that is no NCName (SACM-ID-002), or a reference to no `xmi:id` it
  writes (SACM-REF-001), is `WriteError::Invalid`, and preserved content is
  `WriteError::PreservedContent` (SACM-XMI-006). A tolerant write writes the document as
  held. `WriteError::NoRoot` and `WriteError::UnrepresentableCharacter`, which names the
  `WriteAttribute` [LIB-COMPAT-007, SACM23-XMI-008, LIB-API-010].
- Load, save and load gives an equal document and the same bytes, and documents built
  with `edit` round-trip [LIB-RT-002, LIB-RT-003].

#### The model

- `model::Document`, `ElementId` and the `SacmElement` view: roots, look-up by id, parent,
  children and pre-order traversal; `ElementKind`, `AbstractClass` and `MetamodelPackage`;
  `ElementKind::may_contain` and `child_kinds`, the containment table that reading and
  editing use [LIB-API-002, LIB-API-003, LIB-API-008, LIB-API-012].
- `LangString`, `MultiLangString` and `AssertionDeclaration`, with absent and empty kept
  apart [SACM23-BASE-001, SACM23-BASE-002, SACM23-ARG-003].
- On every element: `name()` (`None` when missing; nothing invents one) and `name_id()`,
  `gid()`, `is_citation()`, `cited_element()`, `is_abstract()` and `abstract_form()`, and
  `descriptions()`, `implementation_constraints()`, `notes()` and `tagged_values()`, whose
  `content()` and `key()` keep their ids [SACM23-BASE-003, SACM23-BASE-004,
  SACM23-BASE-005, SACM23-BASE-006, SACM23-BASE-007, SACM23-BASE-008].
- Argumentation: Claim (with `assertionDeclaration` and `metaClaim`), ArgumentReasoning,
  AssertedInference, AssertedContext, AssertedEvidence, AssertedArtifactSupport,
  AssertedArtifactContext, ArtifactReference and ArgumentGroup, with their ends
  (`sources()`, `target()`, `reasoning()`, `is_counter()`, `structure()`,
  `referenced_artifact_elements()`, `group_members()`) [SACM23-ARG-001, SACM23-ARG-002,
  SACM23-ARG-004, SACM23-ARG-005, SACM23-ARG-006, SACM23-ARG-007, SACM23-ARG-008,
  SACM23-ARG-009, SACM23-ARG-010, SACM23-ARG-011, SACM23-ARG-012, SACM23-ARG-013].
- Artifacts: ArtifactPackage, Property (`properties()`), Technique, Participant, Artifact
  (`version()`, `date()`), Resource (`location()`), Activity (`start_time()`,
  `end_time()`), Event (`date()`), ArtifactGroup and ArtifactAssetRelationship
  (`sources()`, `targets()`). A date-typed value is held exactly as written
  [SACM23-ART-001, SACM23-ART-002, SACM23-ART-003, SACM23-ART-004, SACM23-ART-005,
  SACM23-ART-006, SACM23-ART-007, SACM23-ART-008, SACM23-ART-009, SACM23-ART-010,
  SACM23-ART-011].
- `SacmElement::flag` and `references`, with `Flag` and `ReferenceEnd` (`ALL`,
  `applies_to`, `bounds`); `Document::referrers` lists every reference to an id
  [LIB-API-014, LIB-API-015].
- `compare::semantic_compare` lists every `Difference` in SACM content between two
  documents; `Document: Eq` holds when there is none [LIB-RT-001].
- `diagnostic`: `Diagnostic` (code, severity, matrix row, clause, affected ids, location,
  message), `Severity`, `Code`, `SourceLocation` and `has_errors`. Every code is in the
  catalog `docs/sacm/diagnostics.md` [LIB-DIAG-001, LIB-DIAG-002].
- One `#[non_exhaustive]` error type per operation, implementing `std::error::Error`
  [LIB-API-004]; `sacm::VERSION` and `sacm::SACM_VERSION` [LIB-API-001].

#### Validation

- `validation::validate(&Document) -> Vec<Diagnostic>` reports, sorted and without
  changing the document [SACM23-VAL-018]:
  - an element with no name, an empty `content` or `key`, a language two values share, a
    TaggedValue with no `key`, and a relationship with no source or no target
    (SACM-MULT-001); two descriptions on one element, and an id repeated in a multi-valued
    end (SACM-MULT-001, Warning) [SACM23-VAL-005, SACM23-VAL-010, SACM23-VAL-011,
    SACM23-VAL-022, SACM23-ART-005, SACM23-ART-010];
  - an id that is no NCName (SACM-ID-002) and a repeated `gid` (SACM-ID-003)
    [SACM23-VAL-002, SACM23-VAL-020];
  - a reference to no element (SACM-REF-001), to an element of the wrong kind
    (SACM-REF-002), or into preserved content (SACM-REF-003, Warning) [SACM23-VAL-003,
    SACM23-VAL-004, SACM23-VAL-016, LIB-COMPAT-003];
  - the citation, abstract-form, `asCited` and implementation-constraint rules
    (SACM-CITE-001, SACM-ABS-001, SACM-CITE-002) and ArgumentPackage homogeneity
    (SACM-PKG-001) [SACM23-VAL-007, SACM23-VAL-008, SACM23-VAL-012, SACM23-VAL-019,
    SACM23-VAL-021].

#### Editing

- `edit::apply(&mut Document, impl Into<Operation>)` is all or nothing: a refused
  operation leaves the document unchanged, and a `Mutation` lists the ids created,
  changed and deleted. `edit::check` asks without applying, `edit::preview` shows what a
  delete would do, `apply_preview` refuses a stale preview (`Document::revision()`), and
  `edit::batch` and `apply_all` apply several operations atomically [LIB-EDIT-002,
  LIB-EDIT-003, LIB-EDIT-004, LIB-EDIT-007, LIB-EDIT-013, LIB-EDIT-014].
- Creates for every kind the reader reads: `CreateAssuranceCasePackage`,
  `CreateArgumentPackage`, `CreateArtifactPackage`, `CreateClaim`,
  `CreateArgumentReasoning`, the five asserted relationships, `CreateArtifactReference`,
  `CreateArgumentGroup`, `CreateDescription`, `CreateImplementationConstraint`,
  `CreateNote`, `CreateTaggedValue`, `CreateProperty`, `CreateTechnique`,
  `CreateParticipant`, `CreateArtifact`, `CreateResource`, `CreateActivity`,
  `CreateEvent`, `CreateArtifactGroup` and `CreateArtifactAssetRelationship`. Generated
  ids are `<snake_case kind>_<n>`, deterministic [LIB-EDIT-001, SACM23-ART-001 to
  SACM23-ART-011].
- `SetName`, `SetGid`, `SetFlag`, `SetAssertionDeclaration`, `SetContent`, `SetKey`,
  `SetVersion`, `SetDate`, `SetLocation`, `SetStartTime` and `SetEndTime`;
  `SetReferences`, `AddReference` and `RemoveReference`; `ReorderElements` [LIB-EDIT-008,
  LIB-EDIT-009, LIB-EDIT-010, LIB-EDIT-012].
- `MoveElement` and `CopyElements` (from another document, with clashing ids renamed)
  [LIB-EDIT-017, LIB-EDIT-018].
- `DeleteElement`, with policies for referenced elements, non-empty elements and
  references across packages; a delete never leaves a reference to an element it
  deletes; references that dangled before are left as they were [LIB-EDIT-005,
  LIB-EDIT-006].
- No edit makes a dangling or wrongly typed reference (`EditError::ReferenceNotFound`,
  `WrongReferenceType`), and every `EditError` has a code [SACM23-VAL-004].
- Undo and redo by whole-document copies, as a tested recipe in the `edit` docs
  [LIB-EDIT-011].

#### The CLI

- `sacm version`, `sacm validate <file> [--strict | --tolerant] [--format human|json]`
  (strict by default; exit 0, 1 or 2; a versioned JSON report) and
  `sacm roundtrip <file> [--strict] [-o <out>]` [LIB-CLI-001, LIB-CLI-002, LIB-CLI-003].

#### serde

- The optional `serde` feature: `Serialize` and `Deserialize` for operations, mutations,
  edit errors, diagnostics and the values they carry, and `Serialize` for `Preview`. The
  JSON shape (ADR 0030) follows the crate's semver until 1.0. `Document` and
  `CopyElements` are not serialized [LIB-API-006].

#### Docs

- The crate root and the README show a complete load, inspect, edit, validate and save
  example [LIB-DOC-001].
- docs.rs documents `sacm` with the `serde` feature; the `test-support` API is not shown
  [LIB-QA-005].
- `docs/releasing.md`, the release process [LIB-QA-005].

### Known limits

- Not read yet: the terminology kinds (TerminologyPackage, Category, Term, Expression,
  TerminologyGroup) and `ExpressionLangString` (M4); the interface and binding kinds of
  the four packages, and a package's `interface` end (M5); cross-document references,
  role elements in reference form, and XMI metadata such as `xmi:uuid`, `xmi:label`,
  `xmi:Extension` and `xsi:schemaLocation` (SACM23-XMI-019, SACM23-XMI-027). A strict
  read refuses them as `ReadError::Unsupported` naming the row that reads them; a tolerant
  read keeps them as preserved content.
- Dialects (M6): a root in another SACM revision's namespace (LIB-COMPAT-004) or an
  `xmi:XMI` in another XMI namespace, as the EMF reference implementation writes
  (LIB-COMPAT-002), is refused in both modes, and a foreign container root is
  `InvalidRoot` (LIB-COMPAT-009).
- Encodings other than UTF-8, ASCII and UTF-16 with a byte order mark are refused.
- No configurable limits on input size, nesting depth or element count yet (LIB-SEC-003):
  only the parser's own capacity limits apply, refused as `ReadError::Unsupported` with
  no location. Reading does not recurse, so deep nesting does not overflow the stack
  [LIB-SEC-002].
- The `test-support` feature (`Document::insert` and the diagnostic constructors) is for
  tests and is not part of the stable API.
