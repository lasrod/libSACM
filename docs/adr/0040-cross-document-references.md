# ADR 0040 — Cross-document references

- Status: accepted (the maintainer, 2026-11-19).
- Date: 2026-11-19
- Matrix rows: SACM23-XMI-027, SACM23-XMI-029, SACM23-VAL-014. Amends ADR 0026 (element form),
  ADR 0039 D3 (bounds) and D-ORDER; M5 decisions Q8 (the maintainer, 2026-11-18).

## Context
XMI 2.5.1 §7.10.2: "The URI reference must be of the form URI#id_value, where URI locates the XML
file containing the XML element to link to". Rule 2l: "use the href attribute to specify an XML
element in another document". SACM bindings name participants "external to" their package (§9.4),
and Annex E's binding "externally references its participant packages", so modular cases are
exchanged as several files. Until now a strict read refused such an `href` and a tolerant read
kept it as preserved content, leaving the end empty: a binding of two remote interfaces would be a
false SACM-MULT-001 once SACM23-VAL-014 counts participants. Q8 asks to hold such targets
losslessly, round-trip and count them, and resolve no file the standard does not require.

## Decision
- D1. Read. A reference element's `href` that, trimmed, is `document#fragment` (non-empty
  document part, one `#`, non-empty fragment without `#`, `(` or `)`, no XML white space) is a
  cross-document reference, on every element end but the derived `expression`. `#id` stays an id.
  An `href` that names no element is preserved with SACM-XMI-003. XPointer fragments, an
  `expression`'s `href`, containment proxies and XLink stay Interim under SACM23-XMI-029.
- D2. Hold. `model::Href`, exactly as trimmed, in a second per-end list beside the ids:
  `SacmElement::hrefs(end)`. `references(end)` never holds one. A reference attribute's token is
  always an id.
- D3. Order. Each list keeps document order and repeats. The interleaving of ids and `href`s
  in one end is not kept: a write writes the ids first (amends D-ORDER; SACM orders no end).
- D4. Write. An end holding an `href` is written in element form in both modes: its ids as
  `xmi:idref` elements, then its `href`s (Annex B.2 constraint 10: "hrefs are never used to
  reference elements in the same XMI file"). The schema check does not look at an `href`.
- D5. Count. An `href` is an occurrence of its end. A lower bound counts distinct ids plus
  distinct `href`s (compared as strings), an upper bound every value.
- D6. Validate. An `href` is never SACM-REF-001, -002 or -003. Rules about a value's presence
  count it; rules about the target's kind or place skip it; SACM23-VAL-022 counts repeats.
- D7. Edit. `SetReferences` and `::clear` set the ids and remove the `href`s. `AddReference` and
  `RemoveReference` change ids. Bounds follow D5. A delete never makes an `href` dangle, and
  LIB-EDIT-019 counts `href`s. A copy keeps them; a move changes none. No operation creates
  an `href` yet.
- D8. Compare. `Difference::Hrefs { id, end, left, right }`, after `ParticipantPackages`.
- D9. No resolution. No file is opened, and no URI is normalised or decoded. `document()` and
  `fragment()` let an application load the other file and check it.

## Consequences
Modular files read strictly, write back byte-stable and validate without false findings.
Code that walks `references()` keeps working and never mistakes an `href` for a local id. Code
that needs every value of an end must also read `hrefs()`. Cross-file type and locality checks
are the application's.

## Alternatives considered
An enum of id or `href` in every end (breaks every accessor). The `href` inside `ElementId`
(every caller would see a dangling id). Keeping the interleaving by position (complexity, for an
order SACM does not define). Resolving other files (not required by XMI or SACM; I/O and
security; `xmi::read` takes bytes). SACM-XMI-003 for XPointer (rule 2n points to XPointer, so it
may conform).
