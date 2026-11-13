# 0021. The XMI reader: package content

- Status: accepted. This ADR records decisions made in slices 0026, 0029 and 0030: by the
  maintainer (the ADR 0017 amendments, and reading package content before more editing) and
  by the lead under ADR 0015's interim rule (the minimal options in slice 0029). The
  maintainer asked for them to be recorded on 2026-11-13.
- Date: 2026-11-13
- Matrix rows: SACM23-XMI-012, SACM23-XMI-016, SACM23-XMI-017, SACM23-XMI-010, LIB-API-009

## Context

ADR 0015 sets the read entry point and its interim rule, and ADR 0016 sets the interchange
roots. What the reader does inside a package was decided slice by slice: the roles, the
types, names and declarations. Until now those decisions were scattered across slice briefs,
matrix notes and the writer's ADR (0017). This ADR collects them in one place.

## Decision

- **Children.** A containment child is an element in no namespace, under its parent, whose
  local name is a composite role of the parent's class, spelled as in ptc/22-03-13.
  - Its class comes from `xmi:type` or `xsi:type`, resolved as for roots
    (SACM23-XMI-002, ADR 0016 D2).
  - Without a type attribute, the class is the role's declared type, if that type is
    concrete (XMI 2.5.1 §7.6.3).
  - Each child is read with its id, name and parent, in document order across roles (ADR
    0010 D2). Reading is iterative, with no recursion.
- **Readable kinds.** Only kinds whose model row is verified are read (ADR 0016 D5):
  - an AssuranceCasePackage in `assuranceCasePackage`;
  - an ArgumentPackage in `argumentPackage` or `argumentElement`;
  - a Claim in `argumentElement`.
- **Names.** An element has at most one `name` child, in no namespace, carrying `lang` and
  `content` attributes; absent and empty stay apart (ADR 0012). A type attribute on `name`
  that names a class other than `LangString` is refused at the `name` start tag.
- **`assertionDeclaration`.** On a Claim, the unqualified attribute is read.
  - Its value must be exactly one of the six literals, after XML attribute-value
    normalisation, with no trimming and no case folding (XMI 2.5.1 §8.2.1 rule 8: "a simple
    type derived from string").
  - An explicit `asserted` reads the same as an absent attribute.
  - Any other value is refused as `Unsupported` at the attribute's name, quoting the value,
    until SACM23-VAL-006 gives it a code (ADR 0015 amendment 1).
- **White space.** Text that is all XML white space is accepted wherever only white space
  may appear, however it is written: literally, as CDATA or as character references.
- **Locations.** Every refusal is located at what it refuses: a start tag at its `<`, an
  attribute at its name, and text at its first character that is not white space
  (SACM23-XMI-010).
- **Interim refusals: conforming input not read yet.** Under ADR 0015's interim rule,
  these are refused as `Unsupported` until the row in brackets reads them. They are never
  re-roled, re-typed or dropped. LIB-API-011 keeps this list in the code and gates the
  compliance points on it.
  - Kinds not yet readable (their kind rows), and utility children (SACM23-BASE-004/005).
  - `gid`, `isCitation` and `isAbstract` (SACM23-BASE-006/007/008).
  - An element with no `xmi:id` (SACM23-XMI-020), and an id with XML white space around it
    whose trimmed value is an NCName (SACM23-XMI-025).
  - A second `name` (SACM23-XMI-021), and an `xmi:id` on a LangString (SACM23-XMI-022).
  - Element forms of references (SACM23-XMI-015).
  - XMI metadata (SACM23-XMI-019).
- **Refusals of non-conforming input.** These stay refused by a strict load. Tolerant
  reading of some of them (in brackets) and their catalog codes are assigned by
  [ADR 0022](0022-load-and-save-modes.md); until then they are `Unsupported` too (ADR 0015 amendment 1).
  - A type that does not conform to the role, names an abstract or unknown class, or
    disagrees with another type attribute.
  - An untyped child under an abstract role (strict SACM-XMI-005; tolerant LIB-COMPAT-010).
  - A role the parent's class lacks, or a namespace-qualified role element (tolerant
    LIB-COMPAT-003).
  - The spelling `argumentationElement` (tolerant SACM23-XMI-009).
  - An empty `xmi:id` and a duplicate one are coded refusals now, not `Unsupported`:
    `ReadError::InvalidId` (SACM-ID-002) and `ReadError::DuplicateId` (SACM-ID-001), per
    ADR 0015 amendment 2.
  - An `assertionDeclaration` value that is not a literal (SACM23-VAL-006), the attribute on
    an element that is not an Assertion, and `sacm:assertionDeclaration`.
  - A name given as text (tolerant LIB-COMPAT-008), and any other attribute or text the
    schema does not allow.

**Differences from C++, one line each:**
- C++ matches role elements and `name` by local name in any namespace (`xmi_reader.cpp:1988`);
  here a namespace-qualified one is refused.
- Of two names, C++ keeps the last (`:1182-1218`); here the second is refused.
- C++ reads a name's text as its content (`:805-808`); here it is refused.
- C++ drops a LangString `xmi:id` with an Info diagnostic (`:793-803`); here it is refused.
- C++ reads an ExpressionLangString type on `name`; here it is refused (SACM23-BASE-009).
- C++ reports an unknown declaration literal as SACM-ENUM-001 and keeps `asserted`, losing the
  value (`:1786-1794`); here it is refused until SACM23-VAL-006.
- C++ maps the legacy `justification` to `axiomatic` with a TaggedValue (`:1765-1785`); here it
  is refused as an unknown literal, and any legacy mapping belongs to LIB-COMPAT.
- C++ matches `assertionDeclaration` in any namespace (`node.attribute`); here
  `sacm:assertionDeclaration` is refused.
- C++ maps `undeveloped="true"` to `needsSupport` (`:1796-1815`); here it is an unread
  attribute.

## Consequences

- A document `edit::apply` builds writes, reads back equal, and writes the same bytes again
  (LIB-RT-003).
- Real files with content outside the readable set are refused, never misread. Each later
  row shrinks the list of interim refusals.

## Amendment 1 (2026-11-13): element form of values (slice 0037; SACM23-XMI-014)

- **Both forms are read.** A `name`'s `lang` and `content`, and a Claim's
  `assertionDeclaration`, are read from an element in no namespace named after the property
  (XMI 2.5.1 §7.8.4, production 2b), as well as from the attribute. Both forms give an
  equal model, and `write` writes the attribute form. Value elements may appear anywhere
  among their siblings.
- **The value is the element's character content, exactly.** CDATA and references give
  their characters, and comments and processing instructions are ignored. Nothing is
  trimmed (xsd:string `preserve`; rule 8 for enumerations). An empty element is the empty
  string.
  - An `assertionDeclaration` must still be exactly one literal; any other value stays
    `Unsupported` at the element's start tag until SACM23-VAL-006.
  - A value element with an attribute (`xmi:id`, a type, `xsi:nil`, or the legacy `lang`
    on `content`), an element child, or a namespace is `Unsupported` at that construct.
- **A property given more than once is refused** in both modes: as an attribute and an
  element, or as two elements, even with equal values. Rule 2h says "A specific Property
  shall be serialized in an object either as an XML attribute or as XML elements but not
  both", and the property holds one value.
  - The refusal is `ReadError::RepeatedValue { element, property, locations }`
    (SACM-MULT-001), per ADR 0015 amendment 2.
  - `location()` is the second occurrence, and `locations()` gives every occurrence.
  - Nothing is chosen or dropped (D-PRESERVE). The maintainer chose this on 2026-11-13,
    and chose the same for a second `name` (SACM23-XMI-021).
- **The interim list** loses "element forms of values". The element forms of `gid`,
  `isCitation` and `isAbstract` come with SACM23-BASE-006/007/008, `xsi:nil` with
  SACM23-XMI-026, and references with SACM23-XMI-015.

**Differences from C++, one line each:**
- C++ reads `assertionDeclaration` only as an attribute (`xmi_reader.cpp:1761`); an
  `<assertionDeclaration>` child is unknown content, and the value stays `asserted`.
- C++ reads `<name><content>…</content></name>` as the legacy multi-language name and drops
  `<lang>` (`:1196-1203`); here they are the element forms of `content` and `lang`.
- When a value is given both ways, C++ silently keeps one; here the input is refused.

*Amendment 1 status: accepted (the maintainer, 2026-11-13).*
