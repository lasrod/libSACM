---
slice: 0068
name: modularity-rules
state: done
step: close
rows: [SACM23-VAL-013, SACM23-VAL-014, SACM23-VAL-015, SACM23-VAL-023, SACM23-XMI-027]
mode: pr
base: main (2a7dcbf)
---

# Slice 0068 — Modularity rules and cross-document references

## Scope

M5's last slice.

- **Validation of package interfaces and bindings:**
  - content (VAL-013);
  - `implements` and distinct participants (VAL-014);
  - AssuranceCasePackageInterface content (VAL-015);
  - interface placement (VAL-023, a new Warning, SACM-PKG-002).
- **Cross-document references (XMI-027):** an `href` with a document part on every element reference end is read, held as `model::Href`, written back, compared and counted for bounds. It never resolves another file.
- **Left for later:** the rarer cross-file forms (proxies, an `expression` href, XPointer, XLink) are split to the new M6 row SACM23-XMI-029.

## Basis

- **The standard:** formal/23-05-08 §9.3, §9.4, §10.5, §10.6, §11.1, §11.5, §11.6, §12.4 and §12.5; XMI 2.5.1 §7.10.2 and rules 2l–2n.
- **The maintainer's decisions:** M5 decisions Q2–Q8, and on 2026-11-19 the XMI-027 split and ADR 0040 (accepted).
- **The design brief:** `target/slice-0068-design.md` (git-ignored).

## Log

- 2026-11-19 design: VAL-013, -014, -015 and XMI-027 are reworded and analyzed. XMI-027 moves to M5. VAL-023 (M5, analyzed) and XMI-029 (M6, planned) are new, and CP-001 to -004 depend on XMI-029.
  - ADR 0040 is filed as accepted.
  - D-SEV, D-IFACE-CITE, D-PKG-HOMOG, D-BIND-PART, D-PRESERVE and D-ORDER are updated.
  - Spec defects D22–D27 are added.
- 2026-11-19, the lead's rulings on the design's questions 3 and 4:
  - an interface or binding child with `isCitation` true and no `citedElement` is no citation (Error; §11.5 OCL);
  - `messages-0.1.0.txt` may change for the fixtures this slice reads and checks (behaviour, not English; note on LIB-API-017).
- 2026-11-19 tests: VAL-013 16, VAL-014 8, VAL-015 5, VAL-023 8, XMI-027 17 tests (all cited); 9 stale citations dropped
  (renames and the removed href tests). Red confirmed by the lead (the new tests fail; the gated build fails only on the
  missing API). Snapshot red-0068.tar. Lead rulings: the builder's XMI-015 fixture-test rename stands; the
  `invalid/reference-cross-document-href.sacm.xmi` fixture stays under `invalid/` (a follow-up).
- 2026-11-19 implementation: the builder's 44 hand mutants are all killed (unit tests added for M14, M29). Test changes
  accepted by the lead: the `…_names_no_element_is_preserved_with_xmi_003` test reads through a root without `xlink`
  (the preserved fragment correctly declares bindings in scope); the xtask template-count test is renamed to 106;
  counts and oracles moved (edit_atomic counts `href`s; LIVE_CODES 43; VAL-018 expects the VAL-014 Error of a nameless
  interface or binding root). The lead reworded the design's verified rows plus VAL-018, LIB-API-017/-018, LIB-DIAG-002,
  LIB-API-006 and LIB-EDIT-*, each with a slice 0068 note.
- 2026-11-19 verify round 1 (903c05a): FAIL.
  - B1 (all rows): a let-chain breaks MSRV 1.85; CI's MSRV job is red.
  - B2–B7: untested branches in XMI-027 and VAL-023 (href de-duplication in delete and bounds; the VAL-023 citation
    exemption; the `Hrefs` Display; `repeated_hrefs`; `set_hrefs`'s guards).
  - The fix round: the builder fixes B1–B7, N2, N3, N4 and N10; the lead fixes N1, N5–N9 (matrix wording, with the
    verifier's text), adds B2/B3 to XMI-027 acceptance 15, and updates the follow-ups.
- 2026-11-19 verify round 2 (babc873): PASS on all five rows. Record docs/verification/0068-modularity-rules.md.
