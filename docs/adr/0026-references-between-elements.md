# 0026. References between elements

- Status: accepted (the maintainer, 2026-11-14)
- Date: 2026-11-14
- Matrix rows: SACM23-XMI-005, SACM23-XMI-015, SACM23-BASE-007, SACM23-BASE-008,
  SACM23-VAL-003, SACM23-VAL-007, SACM23-VAL-008. It extends ADR 0020, ADR 0021 and
  ADR 0024, and amends D-VAL-LAYER.

## Context

SACM 2.3 §8.2 gives every SACMElement two flags and two references:
- `isCitation[0..1]=false` and `isAbstract[0..1]=false`;
- `citedElement:SACMElement[0..1]` and `abstractForm:SACMElement[0..1]`.

Thirteen later rows hold references: relationship ends, artifacts, terminology. XMI 2.5.1
writes a reference as an attribute naming the end, whose value is "the XMI ID of each
referenced object, separated by a space" (rule 2j, xsd:IDREFS). It may also be an element
`<end xmi:idref="id"/>` or `<end href="#id"/>` (production 2c, rules 2l and 2n): "either as
an XML attribute or as XML elements but not both" (rule 2h).

## Decision

- **A reference is an unresolved id.** A single-valued end is `Option<ElementId>`, and a
  multi-valued end is `Vec<ElementId>` in document order.
  - Accessors return ids, and `Document::get` resolves them.
  - In this release: `SacmElement::cited_element()` and `abstract_form()`.
- **Flags are `bool`, with a default of false.** Absent and `false` are the same value (§8.2
  `=false`; XMI omits default values).
  - Read: `true`, `false`, `1` and `0`, after white space is collapsed (xsd:boolean), as an
    attribute or a value element.
  - Any other value is `ReadError::InvalidLiteral` (SACM-ENUM-001, whose catalog meaning
    widens to cover a Boolean's literals).
  - Written only when true, as `"true"`.
  - Accessors: `is_citation()`, `is_abstract()`.
- **The attribute form.**
  - The value is split on XML white space (xsd:IDREFS is a list type). Each token is held
    exactly, with no `#` stripped.
  - An empty value is `Unsupported`.
  - Two or more tokens on a single-valued end is `RepeatedValue`.
- **The element form.**
  - It is read when the element is in no namespace, named after the end, and carries exactly
    one of `xmi:idref` or an unqualified `href="#id"`. The value is trimmed (xsd:IDREF,
    anyURI), and the element holds only white space, comments and processing instructions.
  - It reads equal to the attribute form, and `write` writes the attribute form.
  - Anything else is `Unsupported`:
    - both link attributes, or neither;
    - a type attribute, or another attribute;
    - a cross-document or XPointer `href` (a later row reads cross-document links);
    - child elements or text.
  - An attribute and an element for the same end, or two elements on a single-valued end, is
    `RepeatedValue`.
- **Dangling references are kept.**
  - A reference whose id names no element is read in both modes and written back. This
    covers an id absent from the file, and the id of a `name`, `content`, `key` or `value`.
  - `validate` reports it as SACM-REF-001, an Error (SACM23-VAL-003), with
    `affected = [referrer, missing id]`. So a strict save refuses it, and a tolerant save
    writes it.
- **Generated ids skip referenced ids.** Both in the reader and in edits, so a dangling
  reference never resolves by accident.
- **Write order.** `xmi:type`, `xmi:id`, `gid`, `isCitation`, `isAbstract`, `citedElement`,
  `abstractForm`, then the class's own attributes (superclass first, Annex B.5.2).
- **Comparison.** `Difference::{IsCitation, IsAbstract, CitedElement, AbstractForm}`, after
  `Gid`, comparing ids.
- **The §8.2 rules** (SACM-CITE-001 and SACM-ABS-001):
  - `citedElement` present without `isCitation` true is an Error (OCL). `isCitation` true
    alone gives no finding: §8.2 states only that one direction (SACM23-VAL-007).
  - `abstractForm` on an element whose `isAbstract` is true is an Error (stated flatly).
  - A target that is not abstract, or of another kind, is a Warning ("should")
    (SACM23-VAL-008).
  - A dangling target gives only REF-001.
- **Edits.** Setting the flags and references is LIB-EDIT-008/009.
- **The edit invariant (D-VAL-LAYER).** "References resolve" becomes "an edit never creates
  a dangling reference". A dangling reference read from a file is a content finding that
  stays until fixed. Otherwise no edit could run on such a document.
- **Mechanism.** Crate-private tables of flags and reference ends, which the reader, the
  writer, the id pre-pass and validation iterate. A later end adds one entry, one accessor
  and one `Difference` variant.

**Differences from C++, one line each:**
- C++ strips a leading `#` from attribute values (`xmi_reader.cpp:384, 904`); here a token
  is held exactly, and `#` is read only in `href`.
- C++ also splits on `,` (`:881`); here only on XML white space.
- C++ reads `ref=`, an `href` or `idref` in any namespace, and bare text (`:850-873`); here
  only `xmi:idref` and `href="#id"`.
- C++ drops a cross-document `href` with a Warning (`:855-866`); here it is refused, never
  dropped.
- C++ keeps the last of repeated single-valued references (`:1324`); here they are
  `RepeatedValue`.
- C++ reads anything but `true`/`1` as false (`:395`); here only the xsd:boolean literals
  are read.
- C++ reports a dangling reference on read and again in validate (`:2332`,
  `validate.cpp:344`); here only in validate.
- C++ generated ids avoid only existing ids; here they also avoid referenced ids.
- C++'s catalog and our earlier docs stated the citation rule in both directions; here only
  §8.2's OCL direction is checked.

## Consequences

- Files with citations and abstract forms load, compare and save without loss. A dangling
  reference is visible rather than lost.
- Strict save refuses dangling references and the §8.2 Errors. The release notes list them
  (ADR 0022).
- Delete operations (LIB-EDIT-004/005) must decide what happens to a reference whose target
  is deleted.
