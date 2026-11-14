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
- `validation::validate` reports an empty `content` or `key`, each language two values
  of one share, and a TaggedValue with no `key`, as SACM-MULT-001 Errors on the
  UtilityElement; and a `content`, `key` or `value` `xmi:id` that is not an NCName as
  SACM-ID-002 [SACM23-VAL-010, SACM23-VAL-020].

### Changed

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
- `xmi::read` reads descriptions, notes, implementation constraints, tagged values and
  `gid`, which it refused as `Unsupported` before [SACM23-BASE-004, SACM23-BASE-005,
  SACM23-BASE-006].
- Reading a file with many diagnostics locates them in one pass over the text.
- The crate root and README show a load, inspect, edit, validate and save example
  [LIB-DOC-001].
- Simplified the harness: removed CI YAML pin tests and the mutation-exclusions command.
