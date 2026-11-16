# Follow-ups

Non-blocking findings from verification, docs review and Copilot (ADR 0006 amendment 3).
A docs slice clears them every few slices; add new ones at the end.

| Source | Where | Finding |
|---|---|---|
| Verification 0051 | SACM-XMI-005 fix advice | An untyped role element with `xlink:href` or `xmi:href` is rightly `MissingType` (XMI.xsd LinkAttribs allows only an unqualified `href`), but the fix advice does not say that a qualified `href` is not a reference form. |
| Verification 0051 | `.github/workflows/ci.yml` `previous` step | It filters by event but not by base. A `pull_request` run tests the merge commit, so a success against an older base still vouches for the skipped jobs (follow-up 46 asked for the base too). |
| Verification 0051 | `xmi/refusal.rs` `OtherSacmNamespace` | Any root in a non-SACM namespace, even a non-SACM document, is labelled a LIB-COMPAT-004 tolerance; only a SACM-like namespace should be (LIB-COMPAT-004 says SACM-XMI-002 for those). |
| Verification 0051 r2 | `xmi/refusal.rs` `XmiMetadata` docs (about line 78) | They say `href` on a `name`, `description` or other text names XMI-019, but a Description is a role element, so `<description href=…>` names XMI-027. Drop `description`. |
| Verification 0051 r2 | `xmi/roots.rs` `role_metadata` vs `proxy_link` | A role element with both `href` and `xmi:idref`: a typed one takes the first in document order, an untyped one under an abstract role prefers `xmi:idref`. Both refuse and nothing is lost; pick one rule. |
| Verification 0051 r2 | `content_edits.rs` LIB-EDIT-012 timing test | Its name and message say "linear", while the code is n log n (the CHANGELOG says so). Rename it when that file is next touched. |
