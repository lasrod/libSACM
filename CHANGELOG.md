# Changelog

All notable changes to this project are documented here. The format follows
[Keep a Changelog](https://keepachangelog.com/en/1.1.0/), and the project adheres to
[Semantic Versioning](https://semver.org/spec/v2.0.0.html). Before 1.0, a breaking
change bumps the minor version and is listed under **Breaking**. A strict `xmi::write`
checks schema validity only, so a new validation rule never changes what it writes. A new
strict read refusal, or a new strict write refusal, is listed under **Changed**.

## [Unreleased]

### Breaking

- `ReadOptions::default()` reads in strict mode, as `WriteOptions::default()` writes. Ask
  for a tolerant read with `with_mode(Mode::Tolerant)` [LIB-API-020].
- The arguments that hold English prose are `ArgumentValue::Prose`, not `Text`: `error` of
  `SACM-SEC-002/unheld-*`, `expected` of `SACM-XML-001/declaration-character` and
  `/unexpected-character`, `encoding` of `SACM-XML-002/byte-order-mark` and
  `/detected-encoding`, and `attribute` of `SACM-XML-003/unrepresentable-character`. Their
  JSON and every `Display` are unchanged [LIB-DIAG-004].
- A `Diagnostic` read with serde refuses an empty `requirement_id` [LIB-DIAG-005].
- `ElementId` now compares with `str` and `&str`, so the element type of an empty array is
  no longer inferred: `assert_eq!(mutation.created_ids(), [])` needs
  `[] as [ElementId; 0]`, or `.is_empty()`; `id == x.into()` likewise needs the target
  type [LIB-API-013].

### Added

- `ArgumentValue::Prose` and `ArgumentValue::as_prose` [LIB-DIAG-004].
- The templates `SACM-XMI-011/emf-version-on-root` and `/emf-version-on-xmi-element`, which
  name a `version` in EMF's XMI namespace as the file writes it [SACM23-XMI-032].
- `Diagnostic::specification()` and `diagnostic::Specification` name the document a
  clause is in: SACM 2.3 or XMI 2.5.1. A diagnostic's JSON gains `specification` after
  `clause` [LIB-API-013].
- `Document::is_id_in_use`: whether a create may use an id [LIB-API-013].
- `MultiLangString::index_for_lang` (the same language ignoring ASCII case, then the same
  primary subtag, then the first value with text), `MultiLangString::values_mut`, and
  `From<&str>` and `From<String>` for `MultiLangString` [LIB-API-013].
- `ElementId` compares with `str` and `&str` [LIB-API-013].
- `SacmElement::tagged_value(key)`, `Mutation::created_id()` and `BatchMutation::is_empty()`
  [LIB-API-013].
- `WriteError::UnrepresentableCharacter` gains `position`, the index of the value the
  character is in. Its JSON gains `position` after `attribute` [LIB-API-013].
- `SacmElement::ancestors()` and `SacmElement::descendants()` [LIB-API-021].
- `edit::SetTaggedValue` sets the content of the tagged value with a plain key, and creates
  it if there is none [LIB-EDIT-023].
- `edit::preview_batch` and `edit::preview_all` return what a batch would do, the delete
  cascade included, without changing the document [LIB-EDIT-024].
- `compare::changes(before, after)` and `compare::Changes`: the ids created, changed and
  deleted between two documents, in the order of a batch's net lists, and whether the order
  of the roots or the content outside every element changed [LIB-RT-005].
- `diagnostic::CATALOG_JSON`, also shipped as `src/diagnostic/catalog.json`: every code with
  its severities, and every message template with its code, carrier, arguments and English,
  as JSON [LIB-DIAG-006].

### Changed

- A tolerant read of a `version` in EMF's XMI namespace gives `SACM-XMI-011/emf-version-on-root`
  or `/emf-version-on-xmi-element`, not `/version-on-root` or `/version-on-xmi-element` as
  in 0.4.0 [SACM23-XMI-032].
- An unrepresentable character in a value of a `content`, `key` or `location` gives the
  template `SACM-XML-003/unrepresentable-character-in-value`, whose message names the
  value's index, not `SACM-XML-003/unrepresentable-character` [LIB-API-013].
- A `Diagnostic` read from JSON without `specification`, as 0.4.0 wrote it, has
  `specification() == None`, so it is no longer `==` to the diagnostic the crate now gives;
  a `WriteError::UnrepresentableCharacter` in a value read without `position` likewise
  [LIB-API-013].
- `Diagnostic::requirement_id()` is documented as never empty. Only the crate builds a
  `Diagnostic`; the `test-support` constructors are for tests [LIB-DIAG-005].

## [0.4.0] - 2026-11-26

### Breaking

- `xmi::read` refuses, in both modes, a start tag that declares the default namespace
  (`xmlns`) or the prefix `xml` twice, as `ReadError::Malformed` (SACM-XML-001;
  `SACM-XML-001/duplicate-attribute`, or `/duplicate-prefix` for `xmlns:xml`), as
  XML 1.0 §3.1 requires; it read such a file [LIB-SEC-008].
- `xmi::read` now refuses, by default and in both modes, input over 32 MiB, nested deeper
  than 256, with more than 1,000,000 XML nodes, with more than 256 namespace bindings in
  scope at an element, or with more than 256 attributes on one element, the first namespace
  declaration of each prefix not counted, as `ReadError::LimitExceeded`; read a trusted file
  with higher `Limits` [LIB-SEC-003, LIB-SEC-007].
- A strict read's refusal of a root in a SACM-like namespace (`SACM-XMI-002/other-namespace`)
  now ends `(strict mode never reads it; LIB-COMPAT-004 reads it in tolerant mode)`, where it
  ended `(… is to read it in tolerant mode)` [LIB-COMPAT-004].
- A package-kind root in no namespace, or in one whose URI does not contain `sacm`, is
  `ReadError::InvalidRoot` (SACM-XMI-001, templates `SACM-XMI-001/no-namespace` and
  `/not-a-sacm-namespace`) in both modes, where it was `Unsupported` with SACM-XMI-002
  [LIB-COMPAT-004].
- The template `SACM-XMI-002/no-namespace` is removed [LIB-COMPAT-004].
- A tolerant read now opens a document whose roots are in an older SACM revision's
  namespace or a SACM-like one, which it refused [LIB-COMPAT-004].
- The English of `SACM-XML-002/byte-order-mark`, `/detected-encoding` and
  `/declared-encoding` names ISO-8859-1 among the encodings read [LIB-COMPAT-016].
- `ReadError` has the variant `LimitExceeded`; the enum is `#[non_exhaustive]`, so only a
  match without a fallback arm breaks [LIB-SEC-003].
- XMI metadata is no longer preserved content: a strict read no longer refuses it and a
  strict write no longer refuses a document that holds it, `has_preserved_content()` is
  false for it, and no input gives SACM-XMI-014 any more. `Code::XMI_UNREAD_CONTENT`
  stays; the code is never reused [SACM23-XMI-019].
- The templates `SACM-XMI-014/*` are removed: 7 refusal templates and 8 diagnostic
  templates [SACM23-XMI-019].
- An XMI-namespace document element other than `xmi:XMI`, such as `<xmi:Extension>`, is
  `ReadError::InvalidRoot` (`SACM-XMI-001/xmi-namespace-root`) in both modes, where it was
  `Unsupported` with SACM-XMI-014 [SACM23-XMI-019].
- An XMI-namespace element that XMI 2.5.1 does not define where it stands, an `href` or
  `xmi:idref` on a root or a part, an `xmi:id` on the `value` of a `name`, and an identity
  attribute or a type on a reference element do not conform: SACM-XMI-003, kept as
  preserved content, where they were SACM-XMI-014. Under `xmi:XMI`, such an element is
  refused by a strict read (`SACM-XMI-003/wrapper-element`) and kept as the document's
  content by a tolerant one (`/wrapper-element-kept`) [SACM23-XMI-019].
- A role element carrying `xmi:idref` or `href="#id"` is a same-file proxy: the root it
  names moves into its place, so `roots()` and `parent()` change for such documents, where
  the proxy was SACM-XMI-014 [SACM23-XMI-030].
- `compare::Difference` has the variants `Metadata` and `DocumentMetadata`; the enum is
  `#[non_exhaustive]` [SACM23-XMI-019].
- A strict read now returns documents it refused: an XPointer `href`, an `href` as an
  `ExpressionLangString`'s `expression`, and a role element in reference form with an
  `href` to another document (a proxy) are read and held, where they were
  `ReadError::Unsupported` with SACM-XMI-007, and a tolerant read no longer keeps them as
  preserved content. No input gives SACM-XMI-007 any more; `Code::XMI_EXTERNAL_HREF`
  stays, and the code is never reused [SACM23-XMI-029].
- The templates `SACM-XMI-007/*` are removed: 4 refusal templates (`/unread-attribute`,
  `/proxy-child`, `/proxy-child-of-root`, `/href`) and their 4 `-kept` diagnostic
  templates [SACM23-XMI-029].
- An `XLink` attribute on a reference element, and a role element that carries an `href`
  to another document together with an `xmi:id`, `xmi:uuid`, `xmi:label`, another
  attribute, an `xmi:idref` or content, do not conform: SACM-XMI-003
  (`SACM-XMI-003/element-kept`), kept whole as preserved content in both modes, where they
  were SACM-XMI-007 [SACM23-XMI-029].
- `AddReference::id()` and `RemoveReference::id()` return `Option<&ElementId>`, `None`
  for an operation on an href; they returned `&ElementId` [LIB-EDIT-020].
- `implements()` of `CreateAssuranceCasePackageInterface`,
  `CreateArgumentPackageInterface`, `CreateArtifactPackageInterface` and
  `CreateTerminologyPackageInterface` returns `Option<&ElementId>`, `None` when an href
  names the package; it returned `&ElementId` [LIB-EDIT-020].
- `Href::document()` may be empty, for an XPointer into the same document, and
  `Href::as_str()` may start with a type, `sacm:<Class>` and one space; `document()` and
  `fragment()` are those of `Href::uri()` [SACM23-XMI-029].
- The SACM-XMI-010 and SACM-XMI-013 of a same-file proxy that is kept affect its
  container; they named the id the proxy names. That SACM-XMI-010, and the one of a
  cross-file proxy, has the new template `SACM-XMI-010/xsi-type-on-proxy`, whose
  arguments are `element` (the proxy's role), `holder_class`, `holder_id` and `class`; it
  was `SACM-XMI-010/xsi-type`. A resolved proxy's stays `xsi-type`, about the root
  [SACM23-XMI-029].
- `compare::Difference` has the variant `Proxies`, and `edit::EditError` the variant
  `HrefNotHeld`; both enums are `#[non_exhaustive]` [SACM23-XMI-029, LIB-EDIT-020].
- A tolerant read now opens a document in the dialect EMF writes, which it refused with
  SACM-XMI-015: the XMI namespace `http://www.omg.org/XMI` is read as XMI 2.5.1's, with one
  SACM-XMI-015 Warning (`SACM-XMI-015/emf-xmi-namespace-read`), and both saves write XMI
  2.5.1 [LIB-COMPAT-002].
- In a tolerant read, a reference token, a same-document `href` fragment or an
  `expression` written as an EMF fragment path, such as `//@argumentElement.1` or `/1`, is
  read as the id of the element it names, with one SACM-XMI-017 Warning; it was held as
  written [LIB-COMPAT-002].
- The label of a SACM-XMI-015 refusal with a row is now `(strict mode never reads it;
  LIB-COMPAT-002 reads it in tolerant mode)`; it was `(… is to read it in tolerant mode)`
  [LIB-COMPAT-002].
- An `XMI` element in neither XMI namespace, such as XMI 2.4.1's or one in no namespace,
  is refused in both modes with the new template `SACM-XMI-015/not-xmi-element`, which
  takes no `row` and ends `(a limit of this version)`; it was
  `SACM-XMI-015/not-xmi-namespace` [LIB-COMPAT-002].

### Added

- `xmi::Limits` and `xmi::Limit`, `ReadOptions::with_limits` and `ReadOptions::limits`,
  `ReadError::LimitExceeded` and `Code::LIMIT_EXCEEDED` (SACM-SEC-003), with the seven
  templates `SACM-SEC-003/input-bytes`, `/kept-namespaces`, `/kept-text`, `/depth`,
  `/nodes`, `/namespace-copies` and `/namespace-bindings`. The size is checked first, on
  the bytes; the depth and the bindings in scope by a scan before parsing, which also
  counts the bindings the parser copies against the node limit; and the node limit by the
  parser. Everything else a read keeps besides the parsed tree counts against the size
  limit, in one budget: the namespace text preserved content keeps, as each element or
  attribute is kept (`/kept-namespaces`), and the text of each diagnostic, the bytes of its
  text arguments and affected ids, where it is emitted (`/kept-text`) [LIB-SEC-003].
- `Limits::with_max_attributes`, `Limits::max_attributes`, `Limit::Attributes` and the
  template `SACM-SEC-003/attributes`: the scan before parsing refuses an element with more
  attributes than the limit, by default 256, the first namespace declaration of each prefix
  not counted [LIB-SEC-007].
- A tolerant read reads a root in the namespace of SACM 2.0, 2.1 or 2.2 that it
  recognises as SACM 2.3, with one SACM-XMI-008 Warning (`Code::XMI_OTHER_REVISION`,
  template `SACM-XMI-008/other-revision`), and an Event's `occurence` as its date
  (`SACM-XMI-008/occurence`); and a root in another namespace containing `sacm` with one
  SACM-XMI-002 Warning (`SACM-XMI-002/sacm-like`). A strict read refuses both as
  `Unsupported` (`SACM-XMI-008/other-revision-root`, `SACM-XMI-002/other-namespace`),
  labelled `(strict mode never reads it; LIB-COMPAT-004 reads it in tolerant mode)`. Both
  modes refuse a SACM 2.4 root (`SACM-XMI-008/sacm-2-4`). `xmi::write` writes SACM 2.3 in
  the pinned namespace [LIB-COMPAT-004].
- Both modes read input declaring `ISO-8859-1`, in any letter case and with no byte order
  mark; after a UTF-8 byte order mark it is `Malformed`
  (`SACM-XML-001/encoding-contradicts-utf8-bom`) [LIB-COMPAT-016].
- The `write` docs state the bound on its output, "# Output size" [LIB-SEC-003].
- `cargo xtask fetch-spec` also fetches XML Schema Part 2: Datatypes Second Edition,
  pinned by SHA-256 [LIB-TOOL-022].
- XMI metadata is read in both modes with no diagnostic, held exactly as written, compared,
  and written back by both saves where it stood: `xmi:id`, `xmi:label`, `xmi:uuid`, `href`,
  `xmi:idref`, `xmi:type` and the XSI schema locations on `xmi:XMI`; `documentation`,
  `Documentation`, `difference`, `Difference`, `Add`, `Replace`, `Delete`, `extension`
  and `Extension` under it; `xmi:uuid`, `xmi:label` and the schema locations on a root, a
  child, a `name`, a `content`, `key` or `location`, or a `value`; and `xmi:extension` and
  `xmi:Extension` in any of these. Its ids share the document's id space. A copy drops the
  `xmi:uuid` of what it copies, a delete removes the metadata, and a move keeps it
  [SACM23-XMI-019].
- `SacmElement::uuid` and `SacmElement::label` give an element's `xmi:uuid` and
  `xmi:label` [SACM23-XMI-019].
- `compare::Difference::Metadata`, before `PreservedContent`, and
  `Difference::DocumentMetadata`, before `DocumentPreservedContent` [SACM23-XMI-019].
- A strict write checks XMI metadata as XMI.xsd types it: an `xmi:id` that is no `NCName`
  (`SACM-ID-002/metadata-id-not-an-ncname`) and an `IDREF` or `IDREFS` token that names no
  `xmi:id` of the output (`SACM-REF-001/metadata-dangling`) are `WriteError::Invalid`
  [SACM23-XMI-019].
- Same-file proxies: once every root is read, a proxy (`xmi:idref="id"` or `href="#id"`,
  at most one type attribute, nothing else) that names a root of a kind its role and type
  admit moves that root into its place, in both modes, with no diagnostic; chains are
  followed. Otherwise it is kept whole in its container with SACM-XMI-003
  (`SACM-XMI-003/proxy-of-no-root`, `/proxy-wrong-kind`, `/proxy-of-linked-root`,
  `/proxy-of-typed-root`, `/proxy-shared`, `/proxy-cycle`, checked in that order); a root
  that keeps an `href` or an `xmi:idref` of its own, or an XMI 2.5.1 `xmi:type` or an `xsi:type` that only a root can carry, is not moved. Depth is counted as resolved: a moved root takes
  its proxy's depth and its tree shifts with it, and an element nested deeper than
  `Limits::max_depth` refuses the read as `ReadError::LimitExceeded`
  (`SACM-SEC-003/depth`) [SACM23-XMI-030, LIB-COMPAT-020].
- Cross-document links are read in both modes with no diagnostic, held as written, never
  resolved, compared, and written back by both saves: an `href` whose fragment is an
  XPointer (its document part may be empty), an `href` with a type before its URI
  reference (XMI 2.5.1 rule 2n; held as `sacm:<Class> <URI>`, and a typed `#id` is the
  id), and an `href` as an `ExpressionLangString`'s `expression` [SACM23-XMI-029].
- `Href::kind`, `Href::uri` and `Href::is_xpointer` [SACM23-XMI-029].
- Cross-file proxies: a role element of a composite in reference form whose `href` names
  an element of another document, or an XPointer, is held as a `model::Proxy` (its role
  and its `Href`) of its container, `SacmElement::proxies`, in document order. A type
  attribute is held as the href's type. Both saves write each as `<role href="…" />`
  after the container's children. No rule counts a proxy, `RejectIfNonEmpty` counts it as
  contained, a copy keeps it and a move takes it along [SACM23-XMI-029].
- `LangString::expression_href`: the `expression` of an `ExpressionLangString` held in
  another document, which counts as the expression being present; it is written as an
  `expression` reference element in the name or value [SACM23-XMI-029].
- `compare::Difference::Proxies`, reported after `Hrefs` [SACM23-XMI-029].
- `Href::new` makes a cross-document reference from a string, and `HrefError` says why a
  value is refused: `Character` (a character that is not an XML 1.0 `Char`), `WhiteSpace`,
  `Type`, `NoFragment`, `HashInFragment` or `SameDocument` [LIB-EDIT-020].
- Edits create and remove cross-document references: `SetReferences::with_hrefs` (and
  `hrefs`), `AddReference::href`, `RemoveReference::href` (`EditError::HrefNotHeld`,
  SACM-CMD-001, when the end does not hold it) and their `cross_document` accessors; the
  interface creates' `root_href`, `new_href` and `implements_href`; the binding creates'
  `with_participant_package_hrefs` and `participant_package_hrefs`; and
  `LangString::with_expression_href`. Bounds count distinct ids plus distinct hrefs, and
  an `Href` is never checked for existence or type [LIB-EDIT-020].
- JSON (`serde`): an `Href` is a string, checked by `Href::new`; `SetReferences` takes
  `hrefs` and the binding creates `participant_package_hrefs`, each empty by default and
  written only when not empty; `AddReference` and `RemoveReference` take exactly one of
  `id` and `href`, and the interface creates exactly one of `implements` and
  `implements_href`; an `ExpressionLangString`'s `expression` is an id, `null` or
  `{"href": "…"}`; and `EditError::HrefNotHeld` is `{"type", "code", "end", "href"}`
  [LIB-EDIT-020].
- The four model compliance points of SACM 2.3 clause 2 (Assurance Case, Argumentation,
  Artifact and Terminology) are claimed. `docs/sacm/compliance-points.md` is the
  conformance statement: what the claim covers, the schema it is checked against and its
  limits [SACM23-CP-001, SACM23-CP-002, SACM23-CP-003, SACM23-CP-004].
- The EMF dialect: a tolerant read reads the XMI names EMF writes in
  `http://www.omg.org/XMI` and its fragment paths (ADR 0044). `Code::XMI_FRAGMENT_PATH`
  (SACM-XMI-017) and the templates `SACM-XMI-015/not-xmi-element`, `/emf-xmi-namespace`
  and `/emf-xmi-namespace-read`, and `SACM-XMI-017/paths-read` and `/paths-partly-read`
  [LIB-COMPAT-002].
- `sacm convert <IN> -o <OUT>`: reads a file tolerantly, lists every read Warning and
  Error, and writes it as strict SACM 2.3 only when the strict output reads back unchanged;
  it exits 1, writing nothing, when the conversion would lose content [LIB-CLI-004].
- Foreign subclasses: `ReadOptions::with_subclass(namespace, name, kind)` declares a class
  outside SACM 2.3 a subclass of a SACM 2.3 class, and a tolerant read then reads elements
  typed with it as that class, keeping the class as preserved content, which a tolerant
  write writes back as the `xmi:type` and a strict write refuses (ADR 0044 D3, D4).
  `ReadOptions::subclass`, `SacmElement::subclass`, `model::Subclass` and the template
  `SACM-XMI-003/subclass-type-kept` [LIB-COMPAT-017].
- The corpus claim: five files from three tool families (ACEditor, Epsilon and the EMF
  SACM implementation) open, check and write back without loss, the GSN-typed ones with
  the subclass declarations an application would give; interoperability evidence, not
  conformance (`docs/compat/corpus.md`, "What LIB-COMPAT-005 claims") [LIB-COMPAT-005].
- `EditError::KeptRootType` (SACM-CMD-005): `MoveElement::new` and `CopyElements::new`
  refuse to make a child of an element that keeps an `xmi:type` or `xsi:type` as preserved
  content, which only a root can carry; the document is unchanged. Its JSON record is
  `{"type":"KeptRootType","code":"SACM-CMD-005","element":…,"attribute":…,"value":…}`
  [LIB-COMPAT-019].

### Changed

- `SetReferences::new` and `clear` still set an end's ids and remove its hrefs; a set
  that gives the hrefs the end holds, in order, no longer changes the element
  [LIB-EDIT-020].
- A proxy's type must be admitted by its role however it is given: a same-file proxy
  `href="sacm:<Class> #id"` whose class its role does not admit, or that disagrees with
  its type attribute, is kept with SACM-XMI-003, and its class is checked against the
  root it names (`SACM-XMI-003/proxy-wrong-kind`) [SACM23-XMI-029].
- A strict read refuses any name in `http://www.omg.org/XMI` outside the content of an
  `xmi:Extension` or of a Difference's `target`, which XMI.xsd leaves open and both modes
  keep as written, with SACM-XMI-015
  (`SACM-XMI-015/emf-xmi-namespace`, or `/not-xmi-namespace` for the `XMI` element); it
  kept one with SACM-XMI-003 or refused it as an unwritable prefix [LIB-COMPAT-002].
- A strict read refuses a root in an older SACM revision's namespace with SACM-XMI-008,
  where it was SACM-XMI-002 [LIB-COMPAT-004].
- A root in the SACM 2.4 namespace (`…/20260504` or `…/2.4`) is refused in both modes with
  SACM-XMI-008 (`SACM-XMI-008/sacm-2-4`), where it was SACM-XMI-002 [LIB-COMPAT-004].
- A root's type, and each child's type, names a SACM class in any namespace of the root's
  family: for a recognised revision the base and the base with a package name
  [LIB-COMPAT-004].
- A tolerant read of an element carrying `id`, `type`, `uuid` or `label` in both XMI
  namespaces now reads XMI 2.5.1's and keeps EMF's as written, where it read the first in
  document order; a `version` in both is read and ignored, both of them; a tolerant write
  no longer writes such an attribute twice [LIB-COMPAT-019].
- An unprefixed type value is resolved in the default namespace in scope, also when it
  names a subclass declared with `ReadOptions::with_subclass` [LIB-COMPAT-017,
  LIB-COMPAT-019].
- The crates no longer set `homepage`, which named the repository again; crates.io links
  the repository.

### Fixed

- A kept type attribute (`type` in XMI 2.5.1's or EMF's XMI namespace, or `xsi:type`), as
  preserved content or as XMI metadata, keeps the binding of the prefix its value names,
  so the written file declares it; a prefix `sacm` or `xmi` bound to another namespace, or
  no prefix under a default namespace, is written as the holder's `nsN` in the value
  [LIB-COMPAT-019].
- An element that keeps a `type` in EMF's XMI namespace or an `xsi:type`, and none in
  XMI 2.5.1's, is written with its own `xmi:type`, so it reads back as the same class
  and the kept type is never read as a declared subclass [LIB-COMPAT-019].

## [0.3.0] - 2026-11-19

### Added

- `model::Href` and `SacmElement::hrefs(end)`: a reference element's `href` that names an
  element of another document (`document#fragment`, XMI 2.5.1 §7.10.2) is read on every
  element reference end, held as written in a list of the end's own beside its ids, and
  never resolved; `document()` and `fragment()` give its parts [SACM23-XMI-027]. No edit operation creates an `href` yet; code that needs every value of an end must also read `hrefs()`.
- `compare::Difference::Hrefs`, after `ParticipantPackages`, one per end whose `href`s
  differ [SACM23-XMI-027].
- `Code::INTERFACE_PLACEMENT` (SACM-PKG-002), and eight diagnostic templates:
  `SACM-MULT-001/too-few`, `SACM-PKG-001/binding-participant`, `/cited-kind`,
  `/cited-outside`, `/not-a-citation`, `/not-a-citation-or-relationship`,
  `/not-an-interface` and `SACM-PKG-002/outside-implemented-package`
  [SACM23-VAL-013, SACM23-VAL-014, SACM23-VAL-015, SACM23-VAL-023].

### Changed

- With `serde`, a refusal or diagnostic record reads its `arguments` in any key order, as
  JSON that passed through `serde_json::Value` has them; missing, extra, repeated or
  wrongly typed arguments are still refused [LIB-API-019].
- A strict read accepts an `href` with a document part on a reference element, which it
  refused as `Unsupported`; both saves write an end that holds one in element form, its
  ids first, then its `href`s. An `href` counts toward its end's bounds and as an
  occurrence of a single-valued end, and is never SACM-REF-001, -002 or -003
  [SACM23-XMI-027].
- An `href` that names no element (no `#`, an empty fragment, a `#` in the fragment or XML
  white space inside it) keeps its reference element as preserved content with
  SACM-XMI-003 in both modes, where a strict read refused it as `Unsupported`. An XPointer
  `href`, an `href` with a document part as an `expression`, a proxy and `XLink`
  attributes stay `Unsupported` with SACM-XMI-007, now naming SACM23-XMI-029; the English of
  `SACM-XMI-007/href` and `/href-kept` changes, their ids and arguments do not
  [SACM23-XMI-027].
- `SetReferences` and `SetReferences::clear` remove the end's `href`s; `AddReference`,
  `RemoveReference`, `CopyElements` and the delete of a binding left below two
  participants count them [SACM23-XMI-027].
- `validation::validate` reports the content of package interfaces and bindings
  (SACM-PKG-001), a missing `implements` and fewer than two distinct participants
  (SACM-MULT-001), a binding named as a participant (SACM-PKG-001 Warning), the
  sub-packages of an `AssuranceCasePackageInterface` (SACM-PKG-001), and an interface
  outside the package it implements (SACM-PKG-002 Warning) [SACM23-VAL-013, SACM23-VAL-014,
  SACM23-VAL-015, SACM23-VAL-023].

## [0.2.0] - 2026-11-19

### Breaking

- `ReadError::code()` and `WriteError::code()` return `Code`, not `Option<Code>`: every
  refusal has a catalog code [LIB-API-017]. To migrate:

  | 0.1 | 0.2 |
  | --- | --- |
  | `error.code() == Some(Code::ID_DUPLICATE)` | `error.code() == Code::ID_DUPLICATE` |
  | `if let Some(code) = error.code() { … }` | `let code = error.code();` |
  | `error.code().map(Code::as_str)` | `error.code().as_str()` |

  `Unsupported`, and every `WriteError` but `PreservedContent`, now have a code where 0.1
  gave `None`.
- With `serde`, a `LangString` given as a JSON array (`["en","x"]`), which 0.1's derived
  reader accepted, is refused; the documented object form reads as before [SACM23-BASE-009].

### Added

- `diagnostic::Argument`, `ArgumentValue` and `Details`, and `template()` and `arguments()`
  on `ReadError` and `WriteError`: each refusal's sentence form as a stable id such as
  `SACM-XMI-014/unread-attribute`, and its values as named arguments, so an application can word
  it in any language. `docs/sacm/diagnostics.md` lists every template with its arguments
  and English [LIB-API-017].
- Codes SACM-XMI-015, SACM-XMI-016, SACM-XML-002, SACM-XML-003 and SACM-SEC-002, and the
  SACM-XMI-002 constant: `Code::XMI_WRAPPER_NAMESPACE`, `XMI_STRICT_SAVE_INVALID`,
  `XML_ENCODING_NOT_READ`, `XML_UNREPRESENTABLE_CHARACTER`, `PARSER_LIMIT` and
  `XMI_NAMESPACE_NOT_PINNED` [LIB-API-017].
- With `serde`, `ReadError`, `WriteError` and `WriteAttribute` serialize and read back: a
  refusal is `{"type", "code", "template", "arguments", …its fields}` [LIB-API-017].
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
- `ExpressionLangString` (§8.4): a name, or a value of a `content`, `key` or `location`,
  may link an ExpressionElement. `LangString::expression()`, `is_expression_lang_string()`,
  `with_expression(id)` and `without_expression()`; `xmi::read` and `xmi::write` read and
  write it as `xmi:type="sacm:ExpressionLangString"` with an `expression` reference
  [SACM23-BASE-009].
- `ReferenceEnd::Expression` and `SacmElement::expressions()`: the ids an element's name
  and texts link, listed by `Document::referrers` and delete previews. A delete that
  scrubs references makes the text plain and keeps its `lang` and `content`
  [SACM23-BASE-009].
- `EditError::DerivedEnd` (SACM-CMD-008): the reference edits refuse
  `ReferenceEnd::Expression`, which is set through the name or text [SACM23-BASE-009].
- `validate` reports an ExpressionLangString with no `expression` (SACM-MULT-001), one
  with an `expression` and a non-empty `content` (SACM-EXPR-001,
  `Code::EXPRESSION_WITH_CONTENT`, a Warning), and a concrete Expression whose `element`
  names an abstract Term or Expression (SACM-ABS-001) [SACM23-BASE-009, SACM23-VAL-009,
  SACM23-VAL-017].
- With `serde`, an ExpressionLangString is
  `{"type":"ExpressionLangString","lang","content","expression"}`; a plain LangString is
  unchanged [SACM23-BASE-009].
- `Diagnostic::template()` and `Diagnostic::arguments()`: every diagnostic's sentence form
  as a stable id such as `SACM-MULT-001/too-many`, and its values as named arguments;
  `docs/sacm/diagnostics.md` lists every template [LIB-API-018].
- With `serde`, a `Diagnostic` writes `template` and `arguments` after `message`; JSON
  without them, as 0.1.0 wrote, still reads [LIB-API-018].
- The package interfaces and bindings of all four families (§9.3, §9.4, §10.5, §10.6, §11.5,
  §11.6, §12.4, §12.5) are read (as children and as roots, in both modes), written, created,
  edited, compared, copied and moved; `validate` checks their references (SACM-REF-001,
  SACM-REF-002, and SACM-MULT-001 for a repeated id) but not yet a missing `implements`, too
  few participants or their contents (SACM23-VAL-013 to SACM23-VAL-015)
  [SACM23-ARG-014, SACM23-ARG-015, SACM23-ART-012, SACM23-ART-013, SACM23-TERM-007,
  SACM23-TERM-008, SACM23-AC-002, SACM23-AC-003].
- `ReferenceEnd::Interface` (a package's `interface`), `Implements` (an interface's
  `implements`) and `ParticipantPackage` (a binding's `participantPackage`), each typed to
  the holder's family and kept as written: `interface` and `implements` are never synced;
  `SacmElement::interfaces()`, `implements()` and `participant_packages()`;
  `Difference::Interfaces`, `Implements` and `ParticipantPackages`
  [SACM23-ARG-014, SACM23-ARG-015, SACM23-ART-012, SACM23-ART-013, SACM23-TERM-007,
  SACM23-TERM-008, SACM23-AC-002, SACM23-AC-003].
- `edit::CreateAssuranceCasePackageInterface`, `CreateAssuranceCasePackageBinding`,
  `CreateArgumentPackageInterface`, `CreateArgumentPackageBinding`,
  `CreateArtifactPackageInterface`, `CreateArtifactPackageBinding`,
  `CreateTerminologyPackageInterface` and `CreateTerminologyPackageBinding`, as a root or
  under a parent; with `serde`, their JSON fields are `implements` and
  `participant_packages` [SACM23-ARG-014, SACM23-ARG-015, SACM23-ART-012, SACM23-ART-013,
  SACM23-TERM-007, SACM23-TERM-008, SACM23-AC-002, SACM23-AC-003].

### Changed

- `ReadError::Unsupported` and `WriteError::NoRoot`, `UnrepresentableCharacter` and
  `Invalid` have a code; `sacm validate` prints and writes it where 0.1.0 printed `error`
  and wrote `null` [LIB-API-017, LIB-CLI-002].
- A strict read no longer refuses a TerminologyPackage, Category, Term, Expression or
  TerminologyGroup as `Unsupported`, and a tolerant read no longer keeps them as preserved
  content with SACM-XMI-014; references to them are type-checked (no SACM-REF-003)
  [LIB-API-011, SACM23-TERM-001].
- A strict read no longer refuses a `name` or `value` typed `sacm:ExpressionLangString` as
  `Unsupported`, and a tolerant read no longer keeps it as preserved content with
  SACM-XMI-014 [SACM23-BASE-009, LIB-API-011].
- Diagnostics equal in the seven other keys order by template, then by arguments. A
  diagnostic built with `test-support`'s `Diagnostic::new` has its code's text as its
  template, so it is no longer `==` to one the crate emits. `Debug` of a `Diagnostic`
  shows its template and arguments [LIB-API-018].
- A strict read no longer refuses any package interface or binding kind, or a package's
  `interface` end, as `Unsupported`, and a tolerant read no longer keeps them as preserved
  content with SACM-XMI-014: no element kind is refused as not read yet
  [SACM23-ARG-014, SACM23-ARG-015, SACM23-ART-012, SACM23-ART-013, SACM23-TERM-007,
  SACM23-TERM-008, SACM23-AC-002, SACM23-AC-003, LIB-API-011].
- `ScrubReferences` and `DeleteReferencingRelationships` delete an interface left with no
  `implements`, or a binding left with fewer than two distinct participants, with its
  contents; under `RejectIfNonEmpty` such a holder that contains elements refuses the
  delete as `EditError::NotEmpty`, whose `element` then names the holder [LIB-EDIT-019].
- `EditError::TooFewReferences.given` counts distinct ids: a lower bound counts distinct
  ids, an upper bound every id. Nothing changes for an end of 0.1.0 [SACM23-ARG-015,
  SACM23-ART-013].

### Known limits

- Not read yet: cross-document references, role elements in reference form, and XMI
  metadata (SACM23-XMI-019, SACM23-XMI-027). A strict
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
