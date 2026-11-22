# Verification 0073b — Cross-document links

- Date: 2026-11-22
- Row verdict: SACM23-XMI-029: PASS
- Row verdict: LIB-EDIT-020: PASS
- Verifier: `sacm-verifier` (independent, read-only), three rounds.
  - **Round 1, 98493b4:** FAIL for both rows.
  - **Round 2, 9784412:** PASS for LIB-EDIT-020. FAIL for SACM23-XMI-029, on one test gap in the fix round's code.
  - **Round 3, 564f1d5:** PASS for SACM23-XMI-029.

  The maintainer approved the extra fix round on 2026-11-22. CI run
  [44996127249](https://github.com/lasrod/libSACM/actions/runs/44996127249) is on round 3.
- Slice brief: [docs/slices/0073b-cross-document-links.md](../slices/0073b-cross-document-links.md)
- Decisions:
  - [ADR 0040](../adr/0040-cross-document-references.md) amendment 1;
  - M6 decisions Q4 and Q8, and the maintainer's answers of 2026-11-20;
  - the lead, 2026-11-22: `HrefError::Character`, checked first, and the template
    `SACM-XMI-010/xsi-type-on-proxy`, with its arguments in sentence order (ADR 0030
    amendment 3).

## Result

- **SACM23-XMI-029, cross-document links read and written.**
  - The reader holds, in both modes, what it refused with SACM-XMI-007 before:
    - XPointer hrefs;
    - type-prefixed hrefs (XMI 2.5.1 rule 2n), held as `sacm:<Class> <URI>`;
    - cross-file containment proxies, as `SacmElement::proxies()`, written after the container's other children;
    - an `expression` href, as `LangString::expression_href()`.
  - An identity, type, XLink or other attribute on a reference element, and `href` beside `xmi:idref`, are SACM-XMI-003.
  - No refusal is `Interim` any more.
  - A kept proxy's diagnostics affect its container. Its `xsi:type` is `SACM-XMI-010/xsi-type-on-proxy`, which names the role as written, the container and the class.
- **LIB-EDIT-020, hrefs in edits.**
  - `Href::new` accepts what the reader holds, and refuses the rest in a fixed order. It refuses any character XML 1.0 cannot represent first.
  - Hrefs can be set, added and removed, with the end's bounds checked. The interface and binding creates take hrefs, and `LangString::with_expression_href` sets an expression href.
  - The JSON forms carry hrefs.
- **How it was shown:**
  - the cited tests;
  - the verifier's hand mutations: 55 viable in round 1, every branch added by the fix round in rounds 2 and 3;
  - the CI PR-diff mutants shards. One survivor is triaged as equivalent (`argumentation.rs:1393:38`).

## Findings

| Blocking / non-blocking | Row | Finding | Resolution |
|---|---|---|---|
| Blocking | LIB-EDIT-020 | `Href::new` accepted characters XML 1.0 cannot represent, so a strict save wrote output that did not read back. | `HrefError::Character`, checked first; tests (round 1). |
| Blocking | LIB-EDIT-020 | A missed serde mutant (`ExpressionVisitor::expecting`). | A test of the error text (round 1). |
| Blocking | SACM23-XMI-029 | The `xsi:type` argument of a kept proxy was untested. | Full-argument assertions (round 1). |
| Blocking | SACM23-XMI-029 | SACM-XMI-010 on a proxy paired the proxy's class with the container's id. | The new template `SACM-XMI-010/xsi-type-on-proxy` (round 1). |
| Blocking | SACM23-XMI-029 | The proxy write order was untested. | `sacm23_xmi_029_a_proxy_is_written_after_the_placed_and_preserved_children` (round 1). |
| Blocking | SACM23-XMI-029 | The new template's `element` argument (the role as written) was untested for a cross-file proxy. | An assertion with a spelled role (round 3). |
| Non-blocking | both | The equivalent mutant, spec defect D28, and doc wording. | Fixed in round 1 with the verifier's text. |
| Non-blocking | LIB-EDIT-002 | The randomized atomicity test does not cover the href forms. | `docs/follow-ups.md`. |
