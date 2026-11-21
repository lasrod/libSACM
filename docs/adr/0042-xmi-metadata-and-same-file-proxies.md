# ADR 0042 — XMI metadata and same-file proxies

- Status: accepted (the maintainer, 2026-11-19: M6 decisions Q2 and Q3; 2026-11-20: a link on a
  definition is SACM-XMI-003).
- Date: 2026-11-20
- Matrix rows: SACM23-XMI-019, SACM23-XMI-030.
- Amends: ADR 0033 ("What stays refused"), ADR 0035 D1, D2 and amendment 1, and D-PRESERVE.

## Context

XMI 2.5.1 expects tools "to store the extension information and export it again" (§7.3.3).
It says any modifier "must preserve the original UUID" (§7.13), and that a label "is ignored on
import" (§7.6.1). OMG's own ptc/22-03-13 opens with `xmi:documentation`.

Until now this conforming content was refused by a strict read and kept with SACM-XMI-014, so
a strict save could not write it. Separately, §7.10.1 makes every element "either a definition
or a proxy", and says that "the definition of the proxy is replaced by the referenced element".

## Decision

- **D1. What metadata is.**
  - On `xmi:XMI`: the attributes XMI.xsd gives it (id, label, uuid, href, idref, type), and
    the two XSI schema locations.
  - Under `xmi:XMI`: the XMI-namespace elements documentation, Documentation, difference,
    Difference, Add, Replace, Delete, extension and Extension.
  - On every object (a root, a child, a name, a content, key or location, a value): uuid,
    label and the schema locations.
  - Inside every object: extension and Extension.
  - XMI.xsd declares no global `Documentation` (a spec defect; §7.5.3 names
    `xmi:Documentation`); it is read and written as the `Documentation` type.
- **D2. How it is kept.** It is held exactly and written back by both saves, with no read
  diagnostic. It is never preserved content, so `has_preserved_content()` ignores it.
  - Attributes follow the typed ones and precede preserved ones.
  - Bindings are declared once per prefix.
  - Children are placed by their typed index, before preserved content at the same index.
- **D3. Typed surface.** `SacmElement::uuid()` and `label()` are read-only. No edit sets
  metadata.
- **D4. What is not metadata.**
  - Any other XMI-namespace element is SACM-XMI-003. Under `xmi:XMI`, a strict read refuses
    it as a defect.
  - An XMI-namespace document element other than `xmi:XMI` is `InvalidRoot`.
  - `href` and `xmi:idref` on a definition (a root or a text part) are SACM-XMI-003,
    because production 2a gives a definition no LinkAttribs.
  - Identity attributes on a reference element are SACM-XMI-003 (production 2c, rule 4e).
- **D5. Ids.** Metadata ids are in the document's id space, handled as preserved ids are:
  DuplicateId, never generated, ReferenceNotFound, SACM-REF-003.
  - A strict save checks them as NCNames (SACM-ID-002).
  - It also checks the XMI.xsd IDREF and IDREFS attributes of metadata (SACM-REF-001).
  - Other content inside an extension is not checked.
  - Differences are never applied (§2.3.1, an optional compliance point).
- **D6. Edits.**
  - A copy drops the `xmi:uuid` of each copied element and part, because "if the same value
    is used in multiple XML elements, then they are all deemed to reference the same MOF
    element" (§7.6.1). Its other metadata is copied, and its ids must be free.
  - A delete removes the metadata. A move keeps it.
  - A part's metadata follows the part's `xmi:id`, by position.
- **D7. Compare.** Two new differences: `Difference::Metadata { id }` and
  `Difference::DocumentMetadata`.
- **D8. Same-file proxies.** A role element that carries one link naming an id of this
  document is a proxy. It may carry one type attribute and holds no content.
  - **Resolved:** when the id names a root of a kind the role and the type admit, the root
    moves into the proxy's place. Chains are followed, and success gives no diagnostic.
  - **Kept:** for a target that is no root, a wrong kind, a root that itself carries a kept
    `href` or `xmi:idref`, two proxies naming one root, or a cycle, every proxy involved
    stays with SACM-XMI-003 and nothing moves. A root with a kept link would not read back
    as a definition once nested.
  - **Depth:** depth is counted as resolved. A moved root takes its proxy's depth and its
    tree shifts with it; an element deeper than `max_depth` refuses the read as
    `LimitExceeded` (SACM-SEC-003), in both modes. Otherwise a chain of flat roots would
    write a file the reader refuses.
  - D-RT holds: a write writes the nested form.
- **D9. Bookkeeping.** `Unread::XmiMetadata` and the SACM-XMI-014 templates are removed. The
  code constant stays.

## Consequences

- Files from MagicDraw, Cameo and EMF with documentation, uuids or extensions open strictly,
  save strictly and stay byte-stable.
- Modular files whose packages link each other within one file open nested.
- The `Interim` list keeps only `CrossDocumentHref`, until ADR 0040 amendment 1.

## Alternatives considered

- Keeping metadata as preserved content: a strict save would stay refused (Q2 a).
- Typed Documentation and Extension: they need a model no one uses yet (Q2 c).
- Holding a same-file proxy as a link: the package would then be both a root and a child
  (Q3 b).
- Picking the first of two proxies that name one root: that is a reader that picks for the
  user (sacm-expert rule 4).
