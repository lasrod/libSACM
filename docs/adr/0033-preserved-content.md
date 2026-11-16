# 0033. Preserved content

- Status: accepted (the maintainer, 2026-11-15)
- Date: 2026-11-15
- Matrix rows: LIB-COMPAT-003, SACM23-XMI-008, LIB-API-011. It amends D-PRESERVE and
  D-ROOT-NAME and extends ADR 0015, ADR 0016, ADR 0017, ADR 0020, ADR 0021, ADR 0022 and
  ADR 0031.

## Context

D-PRESERVE says SACM content is never silently dropped. Until now, everything the reader did
not read was refused as `ReadError::Unsupported`, so a vendor attribute made a file
unloadable. XMI 2.5.1 §7.3.3 expects tools "to store the extension information and export it
again". The maintainer decided on 2026-11-14 that a strict load keeps unknown content with an
Error. On 2026-11-15 the maintainer decided that only content that does not conform is
preserved.

## Decision

- **What is preserved.** Content that neither the SACM XML Schema nor the XMI 2.5.1
  production rules allow where it stands, and that no tolerance row reads, is kept exactly as
  written. That covers:
  - attributes the element's class lacks;
  - child elements in another namespace, or under a role the class lacks, and any child of a
    UtilityElement;
  - a role child whose type names no concrete class, is not admitted by the role, or
    disagrees with its other type attribute;
  - a defect inside a `name`, `content`, `key`, value or reference element, which preserves
    that whole child and leaves the property absent;
  - non-white-space text in element-only content;
  - an empty `citedElement=""`;
  - a root's type attribute naming another class;
  - `xmi:version` on a child.
- **Both modes read and keep it.** Each construct gives one SACM-XMI-003
  (`Code::XMI_UNKNOWN_CONTENT`): a Warning in tolerant mode, an Error in strict mode.
  - Nothing inside a preserved subtree is read or reported.
  - Its `xmi:id`s are still checked: an empty one is `InvalidId`, a repeated one
    `DuplicateId`.
- **What stays refused.** Conforming content this version does not read yet is still
  `ReadError::Unsupported`. Examples are the artifact and terminology kinds, interfaces,
  bindings, `xmi:Extension`, `xmi:uuid`, cross-document `href` and ExpressionLangString
  names.
  - Each refusal comes from one crate-private list (LIB-API-011) and names its row
    (`Interim`), or is labelled `Tolerance(row)`, `Limit` or `Defect`.
  - Content on the `xmi:XMI` wrapper, and a preserved attribute that binds `sacm` or `xmi`
    to another URI, have nowhere to be kept and are `Defect` refusals.
- **Position and form.**
  - A preserved child keeps its index among the element's children. Attributes come after
    the typed ones, in input order.
  - Namespace URIs, local names, prefixes and values are kept exactly.
  - Fragment text is kept exactly. Top-level stray text is trimmed. Comments and processing
    instructions are dropped, as everywhere in the reader.
  - A top fragment declares every binding in scope where it was read (except `xml` and the
    standard `sacm` and `xmi`). A holder declares the prefixes its preserved attribute names
    use. A prefix used only inside a preserved value is not yet kept; see docs/follow-ups.md
    (a bounded fix is pending).
- **Writing.**
  - A tolerant `xmi::write` writes preserved content back: load, save and load gives an
    equal `Document`, and writing again is byte-identical.
  - A strict write refuses with `WriteError::PreservedContent { elements }`
    (SACM-XMI-006, `Code::XMI_STRICT_SAVE_REFUSED`). It lists each holder once, in document
    order. The write checks run in this order: `NoRoot`, then `PreservedContent`, then
    `Invalid`, then unrepresentable characters.
- **Comparison.** `==` and `semantic_compare` compare preserved content exactly and in order.
  A difference is `Difference::PreservedContent { id }`, which `semantic_compare` reports
  last for that element.
- **Ids and edits.**
  - A preserved id stays in the document's id space: `get` does not find it, `with_id` of it
    is `DuplicateId`, it is never generated, and an edit that would reference it is
    `ReferenceNotFound`.
  - `validate` reports a typed reference to a preserved id as SACM-REF-003
    (`Code::REF_PRESERVED_TARGET`, a Warning), never as REF-001 or REF-002.
  - Deleting a holder removes its preserved content and frees its ids. Typed references to
    those ids are dangling references, handled by the reference policy.
  - A preserved child element counts as content for `RejectIfNonEmpty` (the maintainer,
    2026-11-15).
  - An edit that leaves a reference end holding one or more ids (SetReferences,
    AddReference, RemoveReference) replaces a preserved attribute of the same name on that
    element, such as an empty `metaClaim=""` read before the edit; the explicit edit is the
    repair, and the element is reported changed. Clearing the end, or removing its last id,
    keeps the preserved attribute. A write never emits an attribute twice:
    a typed end beside a preserved attribute of its name (possible straight from a read, such
    as `metaClaim=""` next to `<metaClaim xmi:idref="c2"/>`) is written in the element form,
    and nothing is dropped (lead decisions, slice 0050 verification).
  - No other edit changes or refuses because of preserved content.
  - An explicit "discard preserved content" edit is a later M6 row.
- **API.** Two additions:
  - `SacmElement::has_preserved_content()`;
  - `WriteError::PreservedContent` and `Difference::PreservedContent`, as new variants.

  Preserved content has no constructors and no XML types in the API. Only `xmi::read` creates
  it.
- **Amended decisions.**
  - D-PRESERVE: only content that does not conform is preserved, in both modes. Conforming
    content this version does not read yet is refused.
  - D-ROOT-NAME: a root's type attribute naming another class is preserved and changes
    nothing.

**Differences from C++:**
- **P1.** C++ places a fragment by role, and by index within that role. Role-less vendor
  elements go at the end (`element.h` `PreservedFragment`; `xmi_reader.cpp` ~2068-2078).
  Here every child keeps its index among the typed children.
- **P2.** C++ holds verbatim XML text and `name="value"` strings (`element.h:91-100`). Here
  names and values are held parsed, with namespace, prefix and local name.
- **P3.** C++ preserves only unknown element names and unprefixed or foreign attributes. It
  silently skips unread XMI, XSI and SACM attributes (`xmi_reader.cpp:1515-1560`). Here all
  content that does not conform is kept.
- **P4.** C++ drops a child whose type it cannot resolve (`xmi_reader.cpp:2013-2021`). Here it
  is preserved whole.
- **P5.** A strict C++ load keeps nothing. Here strict keeps the content, with an Error.
- **P6.** C++ compares fragments as a sorted set (`semantic_compare.cpp:40-64`). Here they are
  compared in order and exactly.
- **P7.** C++ writes foreign namespaces on the document element (`xmi_writer.cpp:411-427,
  511-517`). Here each holder or fragment declares its own.
- **P8.** A strict C++ save returns `ok=false` with one SACM-XMI-006 diagnostic, and does not
  validate. Here it is `Err(PreservedContent)`, checked before validation.
- **P9.** C++ invents `sacm.import.*` TaggedValues (`xmi_reader.cpp:1192-1216, 1772-1787`).
  Here nothing is invented.
- **P10.** C++ `compat::adopt_preserved_content` is not adopted. No edit loses preserved
  content, so there is nothing to restore.
- **P11.** The code constant is `XMI_UNKNOWN_CONTENT`, where C++ has `kXmiUnknownElement`,
  because it covers attributes and text too.

## Consequences

- Tests that used content which does not conform as their "not read" sample switch to
  conforming content that is still unread, for example `xmi:uuid`, `<xmi:Extension/>`, an
  `ArgumentPackageInterface` or `href="o.xmi#c"`. The verified rows they belong to are
  re-checked.
- SACM23-XMI-019 widens to identity attributes on reference elements and XMI attributes on
  `name`/`value`. LIB-COMPAT-012's multi-entry names, in a SACM 2.3 file, are preserved.

## Alternatives considered

- **Preserving conforming-but-unread content too.** Rejected by the maintainer: SACM-XMI-003
  would then label valid SACM "unknown".
- **Preserving wrapper-level content.** It has no element to hold it, so it is refused as a
  defect.
- **Holding fragments as raw XML text.** A raw string cannot be compared or re-declared
  correctly once prefixes move.

## Amendment 1 (2026-11-16): tolerant read keeps more; document-level content (ADR 0035)

- **What stays refused:** this section now applies to a strict read. A tolerant read keeps
  conforming content not read yet, with SACM-XMI-014 (SACM-XMI-007 for a cross-document
  `href`), and the other strict refusals as ADR 0035 D1 states.
- **Wrapper-level content:** the rejected alternative "Preserving wrapper-level content" is
  reversed by ADR 0035 D3. A document holds preserved content outside its elements.
- **The prefix exception:** a preserved attribute whose prefix binds `sacm` or `xmi` to
  another URI is kept in tolerant mode. Its namespace URI and local name are kept, and the
  prefix is replaced by the first `nsN` unbound at the holder. This is the one place a
  prefix is not kept exactly.
- **P8:** it now reads "checked before the schema check".

*Amendment 1 status: accepted (the maintainer, 2026-11-16).*
