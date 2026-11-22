---
slice: 0073b
name: cross-document-links
state: in-progress
step: verification
rows: [SACM23-XMI-029, LIB-EDIT-020]
mode: pr
base: main (cbb0c09)
---

# Slice 0073b — Cross-document links

## Scope

The second half of the M6 "metadata and links" slice.

- **SACM23-XMI-029.** The reader holds the links it refused until now: XPointer hrefs,
  type-prefixed hrefs (rule 2n), cross-file containment proxies (`SacmElement::proxies()`) and
  an `expression` href. Identity, type and XLink attributes on a reference element, and `href`
  beside `xmi:idref`, are SACM-XMI-003. No refusal is `Interim` any more.
- **LIB-EDIT-020.** `Href::new`, and edits that set, add and remove hrefs; the interface and
  binding creates take hrefs; `LangString::with_expression_href`; the JSON forms.

It also takes one follow-up of slice 0073a: a diagnostic about a kept same-file proxy affects
its container, not the id the proxy names.

Left out: operations that create or remove a proxy (candidate LIB-EDIT-021), and resolving
other files (ADR 0040 D9).

## Basis

- XMI 2.5.1 §7.6.2, §7.10.1, §7.10.2, production 2c, rules 2l to 2n and 4e; SACM 2.3 §8.4, §9.4.
- M6 decisions Q4 and Q8 (the maintainer, 2026-11-19), and their answers of 2026-11-20: a
  cross-file proxy may carry a type; `AddReference::id()`, `RemoveReference::id()` and the
  interface creates' `implements()` return `Option` (a break for 0.4.0).

## Design

- ADR 0040 amendment 1 (accepted).
- The design brief, `target/slice-0073-design.md` (git-ignored), items tagged [B]: the public
  API, templates, read and write rules, golden L1 and the JSON samples.

## Log

- 2026-11-21 design: SACM23-XMI-029 is reworded and LIB-EDIT-020 is promoted; both are
  analyzed. Candidate LIB-EDIT-021 is new. ADR 0040 amendment 1 is filed as accepted, with A9
  for the 0073a follow-up.
- 2026-11-21 tests: the builder wrote 10 tests for SACM23-XMI-029 and 8 for LIB-EDIT-020, with golden L1. The lead confirmed the red: three ungated tests fail on today's behaviour (SACM-XMI-007 refusals, the affected id `nope`), `sacm23_xmi_029_no_entry_is_interim` on the `Interim` entry, and the gated build (`--cfg slice_0073b_api`) fails only on the missing API. Snapshot red-0073b.tar.
  - The lead's rulings:
    - an unprefixed href type cannot be in a default namespace (a reference element is unqualified), so it is SACM-XMI-003; the positive case is dropped from acceptance 2;
    - L1's `t1` has no `value`, so acceptance 4 expects that one SACM-MULT-001 and no other;
    - a proxy's type must be admitted by its role however it is given (attribute or href prefix);
    - the `id` argument of `SACM-XMI-010/xsi-type` on a kept proxy is the container's id;
    - `Href::new` checks in the table's order, so `"#a b"` is `WhiteSpace`;
    - new `element-kept` diagnostics keep today's row and clause; `xsi:type` on a cross-file proxy is held with its SACM-XMI-010, as on a same-file one;
    - the builder's derived cases stay;
    - the stale tests (SACM-XMI-007, `Interim`, the three accessors, template counts 100 / 105 / 205, the JSON goldens) are changed in step 2.
- 2026-11-22 implementation: step 2 done. The step-1 tests changed only by losing their `slice_0073b_api` gates. The stale tests were rewritten and renamed (10 citations repointed: XMI-015, BASE-009, LIB-API-011 x4, LIB-API-017 x2, LIB-API-018, XMI-027); `lib_api_017_a_cross_document_proxy_in_a_case_is_unsupported...` became `lib_api_017_an_xmi_element_in_no_namespace_is_unsupported...`, since a proxy is no longer refused. The 16 builder-derived tests in `cross_document_link_details.rs` are cited. Both rows are implemented; the local gate is green.
- 2026-11-22 verify round 1 (98493b4): FAIL, both rows. B1: `Href::new` accepted characters XML 1.0 cannot represent, so a strict save wrote output that does not read back. B2: the `xsi:type` argument of a kept proxy was untested. B3: SACM-XMI-010 on a proxy named the proxy's class with the container's id. B4: the proxy write order was untested. B5: a missed serde mutant.
  - The lead's rulings: B1 is fixed by a new `HrefError::Character`, checked first; B3 by a new template, `SACM-XMI-010/xsi-type-on-proxy`, which replaces the ruling on the `id` argument. The XMI-029 and LIB-EDIT-020 requirements, and LIB-COMPAT-010's stale "refused as not read yet" wording, are updated. Spec defect D28 is added, so slice 0074's planned defects become D29 to D32.
  - Fix round 1 is with the builder; the re-verification checks against the round-1 list only.
- 2026-11-22 fix round 1 (9784412): `HrefError::Character`, `SACM-XMI-010/xsi-type-on-proxy` (arguments in sentence order, ADR 0030 amendment 3, accepted by the lead), the B2/B4/B5 tests and the wording.
- 2026-11-22 verify round 2 (9784412): LIB-EDIT-020 PASS, SACM23-XMI-029 FAIL. The one gap left: for a cross-file proxy under a spelled role, no test pins the new template's `element` argument (the role as written). The fix is one test assertion, with no code change. LIB-EDIT-020 stays `implemented` until XMI-029 is verified, because it depends on that row. A round 3 needs the maintainer's OK.
