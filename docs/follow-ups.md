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
| Verification 0051 r3 | `xtask/tests/interim_refusals.rs` | LIB-API-011 [2]'s second half (LIB-COMPAT-004, a Tolerance row, is not in the compliance points' `depends_on`) has no assertion; add one line pinning that no Tolerance row is in their closure. |
| Verification 0057 | `xmi/roots.rs` strict SACM-XMI-013 messages | They end "a tolerant read reads it as argumentElement". That overstates it for an untyped spelled role (a tolerant read gives SACM-XMI-005) and for a spelled role typed `ArtifactAssertedRelationship` (SACM-XMI-003). The name mapping holds. |
| Verification 0057 | `model_spellings.rs` `sacm23_xmi_009_a_strict_read_keeps_each_spelling_…` | The narrowed substrings (e.g. `xsi:nil="true" />`) no longer show the kept element's name. Add `text.contains("<occurece ")` and `text.contains("<argumentationElement ")`. |
| Verification 0057 | D-STRICT vs D-TEXT-MODEL | D-STRICT says a strict read accepts every document that conforms to the SACM XML Schema. A schema generated from ptc/22-03-13 would contain `ArtifactAssertedRelationship` and `occurece`, which a strict read now keeps with an Error (decision 3). No Dn entry records the §12.14 class-name and generalization divergence; only metamodel-inventory.md does. Raise with the maintainer at M3's close. |
