# Verification 0076 — A kept type attribute writes back readable, complete XMI

- Date: 2026-11-25
- Row verdict: LIB-COMPAT-019: PASS
- Row verdict: LIB-COMPAT-017: PASS
- Verifier: `sacm-verifier` (independent, read-only), two rounds.
  - **Round 1, 2acc64d:** FAIL for LIB-COMPAT-019 (two blocking round-trip losses).
  - **Round 2, 8da4d18:** PASS for LIB-COMPAT-019, and for LIB-COMPAT-017, whose rule for an
    unprefixed type value the fix round changed. Matrix and ADR wording was then applied
    with the verifier's text.
- Evidence: CI runs 45545049108 (round 1: PR-diff mutants 48 caught, 13 unviable, 0 missed)
  and 45559078683 (round 2); hand mutations in both rounds; the red snapshot of step 1.
- Slice brief: [docs/slices/0076-kept-type-write.md](../slices/0076-kept-type-write.md)
- Decisions: [ADR 0033](../adr/0033-preserved-content.md) amendment 2 (the maintainer,
  2026-11-25: refuse the edit); [ADR 0044](../adr/0044-the-emf-dialect-and-foreign-subclasses.md)
  amendment 1; D-ROOT-NAME, D-RT, D-XMI-VERSION.

## Result

- **LIB-COMPAT-019.** A tolerant save no longer writes an attribute twice on one start tag,
  and a kept type attribute (`xmi:type`, EMF `type`, `xsi:type`, also as XMI metadata)
  keeps the binding of the prefix its value names. An attribute in both XMI namespaces
  reads XMI 2.5.1's (`id`, `type`, `uuid`, `label`); a `version` in both is read and
  ignored, both of them; an `idref` in both on a SACM element is kept, both of them. An
  element keeping only an EMF `type` or an `xsi:type` is written with its own `xmi:type`.
  `MoveElement` and `CopyElements` refuse, with `EditError::KeptRootType` (SACM-CMD-005),
  to make a child of an element keeping `xmi:type` or `xsi:type`. Every probed tolerant
  write is well-formed and reads back `==`.
- **LIB-COMPAT-017.** An unprefixed type value is resolved in the default namespace in
  scope, also for a declared subclass, as XML Schema resolves a `QName` and as SACM classes
  already were (SACM23-XMI-002). This replaces slice 0075b's "an unprefixed type never
  matches"; one verified test case was reversed and the test renamed, with the lead's
  approval.
- **Tests.** 14 `kept_types.rs` tests; every hand mutant of the new logic killed, except
  equivalents argued in the reports (the `xml`/`xmlns` and empty-prefix guards, which
  roxmltree never reaches, and the child site of default-namespace resolution, which a
  qualified child never reaches).

## Findings

| Blocking / non-blocking | Row | Finding | Resolution |
|---|---|---|---|
| Blocking | LIB-COMPAT-019 | `version` in both XMI namespaces, on a root or `xmi:XMI`, wrote back only EMF's and re-read `!=`. | Both are read and ignored, one SACM-XMI-011 each; control test (round 2). |
| Blocking | LIB-COMPAT-019, -017 | Rule V renamed an unprefixed type value under a default namespace, but a declared subclass treated it as in no namespace, so a re-read found the subclass. | Unprefixed values resolve in the default namespace everywhere; control test; LIB-COMPAT-017 test case (e) reversed and renamed (round 2). |
| Non-blocking | LIB-COMPAT-019 | Requirement (2) claimed `idref` reads XMI 2.5.1's; T1, T5 and the new tests' acceptance text was inexact; xmi.rs, edit.rs tables and the CHANGELOG wording. | The verifier's text. |
| Non-blocking | LIB-COMPAT-019 | ADR 0044 amendment 1 listed `idref` among the attributes read from XMI 2.5.1. | The verifier's text. |
| Non-blocking | LIB-COMPAT-019 | A `read_owned_id` mutant survived round 1. | A control test kills it. |
| Non-blocking | — | SACM-XMI-011's message names an `emf:version` "the xmi:version" (before this slice). | Logged here. |
