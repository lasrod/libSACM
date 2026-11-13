# 0024. Ids read from XMI

- Status: accepted (the maintainer, 2026-11-14)
- Date: 2026-11-14
- Matrix rows: SACM23-XMI-020, SACM23-XMI-022, SACM23-XMI-025; it widens SACM23-VAL-001,
  SACM23-VAL-020 and LIB-RT-001. It answers Q-NO-ID and ADR 0010 amendment 1's open item.

## Context

An `xmi:id` is declared `<xsd:attribute name="id" type="xsd:ID" use="optional"/>` (XMI
2.5.1 §7.6.1), and "the value for xmi:id is unique across all elements within the file".
A `Document` needs an id to hold an element (ADR 0010).

Until now, the reader refused three conforming inputs as `Unsupported`:
- an element with no `xmi:id`;
- an id with white space around it;
- an `xmi:id` on a `name` (a LangString is a class, XMI 2.5.1 §7.8.3).

The maintainer decided their policy on 2026-11-12 and 2026-11-14:
- generate a missing id, report it as Info, and write it;
- read a padded id trimmed, with an Info;
- keep a LangString's id with its owner.

## Decision

- **No `xmi:id`.**
  - Both modes read the element, whether it is a root or a child.
  - Its id is generated in the edit scheme `<snake_case kind>_<n>` (ADR 0019). `n` is the
    smallest number above the kind's counter whose id is not the value of any `xmi:id` in
    the input and was not generated earlier in the read, in document order.
  - A first pass collects every `xmi:id` in the input, so a later explicit id never
    collides. The counters are kept, so a later edit continues above them.
  - Generation is linear in the input.
  - It is reported as Info SACM-XMI-004 (requirement SACM23-XMI-020, clause 7.6.1). The
    affected id is the generated one, and the location is the element's start tag.
  - A `name` without an id gets none. An unqualified `id` attribute is not an `xmi:id`
    (rule 2f).
- **A generated id is an ordinary id.**
  - It is not marked. It is written on save, and it is compared and edited like any other
    id. The diagnostic is its only record.
  - ADR 0013's "nothing invents" concerns SACM properties, such as the name. An `xmi:id` is
    XMI identity, which a `Document` needs in order to hold the element.
- **An id's value is its `xsd:ID` value.**
  - `xsd:ID` derives from `token`, whose white space is collapsed (XML Schema Part 2 §4.3.6).
  - When trimming XML white space (#x20, #x9, #xA, #xD) at both ends changes an `xmi:id` and
    leaves an NCName, the trimmed value is the id: for the element or the LangString, for
    uniqueness, for generation and for references.
  - It is reported as Info SACM-ID-004 (SACM23-XMI-025, clause 7.6.1), at the attribute's
    name, quoting the value as read. It is written trimmed.
  - Any other id is held as written (D-ID). An id that is still not an NCName after trimming
    is reported by `validate` (SACM23-VAL-020), and an empty one is `ReadError::InvalidId`.
  - This replaces ADR 0015 amendment 2's white-space bullet.
- **A LangString's `xmi:id` is kept with its owner.**
  - It is held as `SacmElement::name_id()`, not on `LangString`, with no diagnostic.
  - It shares the file's id space:
    - a repeat is refused on read (`DuplicateId`);
    - a caller id equal to it is refused by edits;
    - generated ids skip it.
  - It is written as the first attribute of `name`, and compared as `Difference::NameId`, so
    equal documents still write equal bytes (ADR 0020).
  - `validate` reports it as SACM-ID-002 when it is not an NCName, with the owner as the
    affected id.
  - No edit sets it yet. An `xmi:id` on a value element (`<content>`) stays refused.
- **Policy text.** D-ID reads "preserved exactly, as `xsd:ID` values". Q-NO-ID becomes
  decision D-NO-ID. The catalog's SACM-XMI-004 row becomes Info.

**Differences from C++, one line each:**
- C++ strict mode reports a missing id as an Error and drops the element; tolerant mode
  reports a Warning (`xmi_reader.cpp:749-771`). Here it is Info in both modes, and the
  element is read.
- C++ generates a kind-independent `generated_<n>` (`:352-365`); here the edit scheme is
  used, so the crate has one scheme. Its whole-file pre-pass (`:2420-2427`) is adopted.
- C++ accepts an `id` in any namespace (`find_attr_local`); here only `xmi:id` counts.
- C++ uses an `xmi:id` raw, without collapsing white space; here an id is its `xsd:ID`
  value, reported as SACM-ID-004.
- C++ drops a LangString's `xmi:id` with an Info SACM-XMI-003 (`:793-803`); here it is
  kept, written and compared.

## Consequences

- Files written by tools that omit ids, pad them, or put ids on names now load, and are
  saved with clean ids.
- Comparing a file without ids with the same file with ids reports every id-less element
  as one-sided. This is identity by id (D-ID, ADR 0020).
- Later LangString slots (descriptions, notes, `TaggedValue.key`) will need their ids kept
  per role and position, in the same way.
