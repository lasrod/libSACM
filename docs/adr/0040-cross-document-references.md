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


## Amendment 1: proxies, XPointer, typed and expression hrefs, and creating hrefs

- Status: accepted. Date: 2026-11-21. Rows: SACM23-XMI-029 and LIB-EDIT-020.
- Source: M6 decisions Q4 and Q8 (the maintainer, 2026-11-19), and the maintainer's answers of
  2026-11-20: a cross-file proxy may carry a type, and the three accessors may return `Option`.
- **A1 (amends D1).**
  - An XPointer fragment (holding `(` or `)`) is read as an `Href`, and its document part may be empty.
  - A type before the URI reference (rule 2n) is one QName of the root's SACM family that names a concrete class, followed by one run of XML white space. It is held as `sacm:<Class> <URI>`. With `#id` it is the id, and the type is not held.
  - An `expression` element's `href` is the ExpressionLangString's `expression_href`.
  - A role element in reference form whose `href` is not `#id` is a cross-file proxy (A2).
  - Identity, type, XLink or any other attribute on a reference element, and `href` beside `xmi:idref` anywhere, are SACM-XMI-003 in both modes (production 2c, rule 4e; §7.6.2 "any one of which may be used at one time").
  - D1's rule against white space still makes every other white space a defect.
- **A2 (amends D2).**
  - `Href` holds the forms of A1, with `kind()`, `uri()` and `is_xpointer()`.
  - `Proxy { role, href }` is held per element in document order by `SacmElement::proxies()`. A proxy's type attribute is held as its href's type and must agree with it.
  - `LangString` holds an expression as an id or an `Href`.
- **A3 (amends D4).**
  - Proxies are written as `<role href="…" />` before the container's end tag.
  - An expression href is written as an `<expression href=…/>` child of the name or value.
- **A4 (amends D5 and D6).**
  - No rule counts a proxy, but `RejectIfNonEmpty` counts it as contained.
  - An expression href counts as present for §8.4 `expression[1]` and SACM-EXPR-001, and is never REF-checked.
  - The type of an href is not checked against its end.
- **A5 (replaces D7's last sentence).**
  - `Href::new` accepts exactly what `as_str` can give, or returns `HrefError`.
  - `SetReferences::with_hrefs`, `AddReference::href` and `RemoveReference::href` (`HrefNotHeld`) are added.
  - The interface creates gain `root_href` and `new_href`, and the binding creates gain `with_participant_package_hrefs`.
  - `LangString::with_expression_href` is added.
  - Bounds follow D5, and an `Href` is never checked (D6).
  - `AddReference::id`, `RemoveReference::id` and `implements()` return `Option` (Breaking, 0.4.0).
- **A6 (amends D8).** `Difference::Proxies { id, left, right }`, after `Hrefs`.
- **A7. JSON.**
  - `Href` is a string checked by `Href::new`.
  - `hrefs` and `participant_package_hrefs` default to empty and are written only when not empty.
  - `AddReference` and `RemoveReference` take one of `id` and `href`; interfaces take one of `implements` and `implements_href`.
  - An `expression` is an id, `null` or `{"href"}`.
- **A8.** `CrossDocumentHref` and the SACM-XMI-007 templates go. The `Interim` list is empty, and its check accepts that.
- **Consequences.**
  - Modular cases split across files open strictly, and their proxies and bindings round-trip.
  - XLink-only files open tolerantly, with SACM-XMI-003.
- **Alternatives considered.**
  - An enum of id or href in every end, which was rejected in ADR 0040.
  - Separate `AddHref` and `RemoveHref` operations: additive, but Q8 chose `AddReference::href`.
  - Keeping a proxy's xmi:type as an attribute: two representations of one fact.
- **A9.** A diagnostic about a kept same-file proxy affects its container, never the id the
  proxy names.
