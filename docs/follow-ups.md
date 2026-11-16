# Follow-ups

Non-blocking findings from verification, docs review and Copilot (ADR 0006 amendment 3).
A docs slice clears them every few slices; add new ones at the end.

| Source | Where | Finding |
|---|---|---|
| Verification 0051 | SACM-XMI-005 fix advice | An untyped role element with `xlink:href` or `xmi:href` is rightly `MissingType` (XMI.xsd LinkAttribs allows only an unqualified `href`), but the fix advice does not say that a qualified `href` is not a reference form. |
| Verification 0051 | `.github/workflows/ci.yml` `previous` step | It filters by event but not by base. A `pull_request` run tests the merge commit, so a success against an older base still vouches for the skipped jobs (follow-up 46 asked for the base too). |
| Verification 0051 | `xmi/refusal.rs` `OtherSacmNamespace` | Any root in a non-SACM namespace, even a non-SACM document, is labelled a LIB-COMPAT-004 tolerance; only a SACM-like namespace should be (LIB-COMPAT-004 says SACM-XMI-002 for those). |
